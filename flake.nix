{
  description = "Open source discord utility bot";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-22.11";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, flake-utils, naersk }:
    let
      lastModifiedDate =
        self.lastModifiedDate or self.lastModified or "19700101";
      version =
        "${builtins.substring 0 8 lastModifiedDate}-${
          self.shortRev or "canary"
        }";

    in rec {
      inherit (flake-utils.lib.eachDefaultSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          naerk-lib = pkgs.callPackage naersk { };
          REV=(pkgs.lib.trace "xxx: " self.shortRev);

        in rec {
          packages.coggiebot = naerk-lib.buildPackage { src = ./.; inherit REV; };

          packages.coggiebot-agent = pkgs.stdenv.mkDerivation rec {
            name = "coggiebot-agent-${version}";
            phases = "buildPhase";
            builder = ./sbin/coggiebot-agent.sh;
            nativeBuildInputs = [
              pkgs.coreutils
              pkgs.nix
              pkgs.git
              packages.coggiebot
            ];
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.default = packages.coggiebot;
          hydraJobs = packages.coggiebot;

          devShell =
            pkgs.mkShell { nativeBuildInputs = with pkgs; [ ffmpeg rustc cargo ]; };
        }))
        packages devShell;

      overlays.default = final: prev: {
        coggiebot = with final;
          final.callPackage ({ inShell ? false }: packages { });
      };

      nixosModules.coggiebot = { pkgs, lib, config, coggiebot, ... }:
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
