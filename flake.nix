{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
      utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        defaultPackage = naersk-lib.buildPackage ./.;

        defaultApp = utils.lib.mkApp {
          drv = self.defaultPackage."${system}";
        };

        devShell = with pkgs; mkShell {
          buildInputs = [ cargo rustc rustfmt pre-commit rustPackages.clippy ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          shellHook = ''

          '';
        };

        nixosModule = pkgs: {config, lib, ...}:
          let
            cfg = config.services.coggiebot;
          in
          with lib;
        {
          options.services."${coggiebot}" = {
            enable = mkEnableOption "coggiebot service";
          };

          config = mkIf cfg.enable {
            systemd.user.services."backup-home" = {
                description = "backup home directory";
                after = [
                  "multi-user.target"
                  "networking.target"
                ];

                ExecStart = "${pkgs.coggiebot}/bin/coggiebot";
                serviceConfig.Type = "simple";
            };
          };
        };
      });
}
