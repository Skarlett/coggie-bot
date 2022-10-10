{
  description = "A Rust web server including a NixOS module";

  # Nixpkgs / NixOS version to use.
  inputs.nixpkgs.url = "nixpkgs/nixos-22.05";
  inputs.import-cargo.url = github:edolstra/import-cargo;
  outputs = { self, nixpkgs, import-cargo }:
    let
      supportedSystems = [ "x86_64-linux" ];
      lastModifiedDate = self.lastModifiedDate or self.lastModified or "19700101";
      version = "${builtins.substring 0 8 lastModifiedDate}-${self.shortRev or "dirty"}";

      forAllSystems = f: nixpkgs.lib.genAttrs supportedSystems (system: f system);
      nixpkgsFor = forAllSystems (system: import nixpkgs { inherit system; overlays = [ self.overlay ]; });
    in {

      # A Nixpkgs overlay.
      overlay = final: prev: {
        coggiebot = with final; final.callPackage ({ inShell ? false }: stdenv.mkDerivation rec {
          name = "coggiebot-${version}";
          src = if inShell then null else ./.;
          buildInputs =
            [ rustc
              cargo
            ] ++ (if inShell then [
              # In 'nix develop', provide some developer tools.
              rustfmt
              clippy
            ] else [
              (import-cargo.builders.importCargo {
                lockFile = ./Cargo.lock;
                inherit pkgs;
              }).cargoHome
            ]);

          target = "--release";
          buildPhase = "cargo build ${target}  --frozen --offline";
          doCheck = true;
          checkPhase = "cargo test ${target} --frozen --offline";
          installPhase =
            ''
              mkdir -p $out
              cargo install --frozen --offline --path . --root $out
              rm $out/.crates.toml
            '';
        }) {};

      };

      # Provide some binary packages for selected system types.
      packages = forAllSystems (system:
        {
          inherit (nixpkgsFor.${system}) coggiebot;
        });

      defaultPackage = forAllSystems (system: self.packages.${system}.coggiebot);
      devShell = forAllSystems (system: self.packages.${system}.coggiebot.override { inShell = true; });

      nixosModules.coggiebot =
        { pkgs, lib, config, coggiebot, ... }: with lib;
        let cfg = config.services.coggiebot;
        in {
          options.services.coggiebot = {
            enable = mkEnableOption "coggiebot service";
            api-key = mkOption {
              type = types.str;
              example = "<api key>";
            };
          };

          config = mkIf cfg.enable {
                nixpkgs.overlays = [ self.overlay ];
                systemd.services.coggiebot = {
                  wantedBy = [ "multi-user.target" ];
                  after = [ "network.target" ];
                  wants = [ "network-online.target" ];
                  environment.DISCORD_TOKEN = "${cfg.api-key}";
                  serviceConfig.ExecStart = "${pkgs.coggiebot}/bin/coggiebot";

                  #preStart = ''
                  #         echo "DISCORD_TOKEN='${cfg.api-key}'" > ${pkgs.coggiebot}/bin/.env
                  #'';

                  #postStop = ''
                  #         rm ${pkgs.coggiebot}/bin/.env
                  #'';
                };
            };
        };

      # Tests run by 'nix flake check' and by Hydra.
      checks = forAllSystems
        (system:
          with nixpkgsFor.${system};
          {
            inherit (self.packages.${system}) coggiebot;

            # A VM test of the NixOS module.
            vmTest =
              with import (nixpkgs + "/nixos/lib/testing-python.nix") {
                inherit system;
              };

              makeTest {
                nodes = {
                  client = { ... }: {
                    imports = [ self.nixosModules.coggiebot ];
                  };
                };

              testScript =
                ''
                  start_all()
                  client.wait_for_unit("multi-user.target")
                  assert "Hello Nixers" in client.wait_until_succeeds("curl --fail http://localhost:8080/")
                '';
              };
          }
        );
    };
}
