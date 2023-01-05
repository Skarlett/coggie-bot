{
  description = "Open source discord utility bot";
  inputs = {
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, flake-utils, naersk }:
    rec {
      inherit (flake-utils.lib.eachDefaultSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          naerk-lib = pkgs.callPackage naersk { };
        in rec {
          packages.coggiebot = naerk-lib.buildPackage { src = ./.; REV=(self.rev or "canary"); };

          packages.updater = pkgs.stdenv.mkDerivation rec {
            name = "update";
            phases = "buildPhase";
            builder = ./sbin/update-builder.sh;
            nativeBuildInputs = [
              pkgs.coreutils
              pkgs.git
              packages.coggiebot
            ];
            coggiebot=packages.coggiebot;
            origin_url="https://github.com/Skarlett/coggie-bot.git";
            branch = "master";
            sysdunit="coggiebotd";
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.starter = pkgs.stdenv.mkDerivation rec {
            name = "start";
            phases = "buildPhase";
            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              mkdir -p $out/bin/
              cat >> $out/bin/$name <<EOF
              #!/bin/sh

              $nix/bin/nix build --refresh --out-link /opt/coggiebot coggiebot
              /opt/coggiebot/coggiebot
              EOF
              chmod +x $out/bin/$name
            '';

            nativeBuildInputs = [ pkgs.coreutils pkgs.nix ];
            nix=pkgs.nix;
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.deploy = pkgs.stdenv.mkDerivation rec {
            name = "coggie-deploy";
            phases = "buildPhase";
            nativeBuildInputs = [
              pkgs.coreutils
              packages.starter
              packages.updater
              packages.coggiebot
            ];

            builder = pkgs.writeShellScript "builder.sh" ''
              mkdir $out
              ln -s ${packages.starter}/bin/start $out/start
              ln -s ${packages.updater}/bin/update $out/update
              ln -s ${packages.coggiebot}/bin/coggiebot $out/coggiebot
            '';

            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.default = packages.deploy;
          hydraJobs = packages.coggiebot;
          devShell =
            pkgs.mkShell { nativeBuildInputs = with pkgs; [ rustc cargo ]; };
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
