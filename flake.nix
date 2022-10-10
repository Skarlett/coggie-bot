{
  description = "Open source discord utility bot";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-22.05";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, flake-utils, naersk }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux"  ];
      lastModifiedDate = self.lastModifiedDate or self.lastModified or "19700101";
      version = "${builtins.substring 0 8 lastModifiedDate}-${self.shortRev or "dirty"}";
      forAllSystems = f: nixpkgs.lib.genAttrs supportedSystems (system: f system);
      nixpkgsFor = forAllSystems (system: import nixpkgs { inherit system; overlays = [ self.overlay ]; });

    in rec {
      inherit(flake-utils.lib.eachDefaultSystem(system:
        let
          pkgs = import nixpkgs {
            inherit system;
          };
          naerk-lib = pkgs.callPackage naersk {};
        in rec {
          packages.coggiebot = naerk-lib.buildPackage {
            src = ./.;
          };
          packages.default = packages.coggiebot;

          devShell = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [ rustc cargo ];
          };
        }
      )) packages devShell;

      overlays.default = final: prev: {
        coggiebot =
          with final;
          let pkgs = import <nixpkgs> { inherit system; };
          in final.callPackage ({ inShell ? false }: packages {});
      };

      # hydraJobs = forAllSystems (system: self.packages.${system}.coggiebot);
      # packages.default = forAllSystems (system: self.packages.${system}.coggiebot);
      # devShell = forAllSystems (system: self.packages.${system}.coggiebot.override { inShell = true; });

      nixosModules.coggiebot =
        { pkgs, lib, config, coggiebot, ... }:
        with lib;
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
            systemd.services.coggiebot = {
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];
              wants = [ "network-online.target" ];
              environment.DISCORD_TOKEN = "${cfg.api-key}";
              serviceConfig.ExecStart = "${pkgs.coggiebot}/bin/coggiebot";
              serviceConfig.Restart = "on-failure";
            };
          };
      };

    };
}
