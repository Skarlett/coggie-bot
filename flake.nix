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
          install_dir="/var/coggiebot";
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

              source $install_dir/.env
              $nix/bin/nix build --refresh --out-link $install_dir/result coggiebot
              $install_dir/result/coggiebot

              EOF
              chmod +x $out/bin/$name
            '';

            nativeBuildInputs = [ pkgs.coreutils pkgs.nix ];
            nix=pkgs.nix;

            install_dir="/var/coggiebot";
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.coggiebotd = pkgs.stdenv.mkDerivation rec {
            name = "coggiebotd.service";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              cat >> $out <<EOF

              [Unit]
              Description=Coggie bot
              Documentation=

              Wants=network.target
              After=network.target

              [Service]
              User=coggiebot
              Group=coggiebot
              KillMode=none
              SuccessExitStatus=0 1

              PrivateDevices=true
              NoNewPrivileges=true
              PrivateTmp=true

              WorkingDirectory=${start-script}
              ExecStart=${start-script}/bin/start

              [Install]
              WantedBy=multi-user.target

              EOF
              chmod 755 $out
            '';

            nativeBuildInputs = [ pkgs.coreutils packages.starter ];
            start-script=packages.starter;

            install_dir="/var/coggiebot";
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.coggiebotd-update = pkgs.stdenv.mkDerivation rec {
            name = "coggiebotd-update.service";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              cat >> $out <<EOF
              [Unit]
              Description=Automatically update coggiebotd.
              Wants=bookmark-bot-update.timer

              [Service]
              Type=oneshot
              ExecStart=${update-script}/bin/update

              [Install]
              WantedBy=multi-user.target
              EOF
              chmod 755 $out
            '';

            nativeBuildInputs = [ pkgs.coreutils packages.updater ];
            update-script=packages.updater;
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.coggiebotd-update-timer = pkgs.stdenv.mkDerivation rec {
            name = "coggiebotd-update.timer";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              cat >> $out <<EOF
              [Unit]
              Description=automatically run self update checks on coggiebotd

              [Timer]
              OnBootSec=15min
              OnUnitActiveSec=15min

              [Install]
              WantedBy=timers.target

              EOF
              chmod 755 $out
            '';

            nativeBuildInputs = [ pkgs.coreutils ];
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.coggiebotd-bootstrap-activate = pkgs.stdenv.mkDerivation rec {
            name = "coggiebotd-bootstrap";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              mkdir -p $out/bin
              cat >> $out/bin/$name <<EOF
                #!/bin/sh
                systemctl stop coggiebotd.service
                systemctl stop coggiebotd-update.service
                systemctl stop coggiebotd-update.timer

                ln -sf ${coggiebotd} /etc/systemd/system/coggiebotd.service
                ln -sf ${coggiebotd-update} /etc/systemd/system/coggiebotd-update.service
                ln -sf ${coggiebotd-update-timer} /etc/systemd/system/coggiebotd-update.timer

                systemctl daemon-reload
                systemctl start coggiebotd.service
                systemctl start coggiebotd-update.service
                systemctl start coggiebotd-update.timer
              EOF
              chmod +x $out/bin/$name
            '';

            nativeBuildInputs = [ pkgs.coreutils pkgs.nix ];
            coggiebotd=packages.coggiebotd;
            coggiebotd-update=packages.coggiebotd-update;
            coggiebotd-update-timer=packages.coggiebotd-update-timer;
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
              packages.coggiebotd-bootstrap-activate
            ];

            builder = pkgs.writeShellScript "builder.sh" ''
              mkdir -p $out/bin $out/etc
              ln -s ${packages.starter}/bin/start $out/start
              ln -s ${packages.updater}/bin/update $out/update
              ln -s ${packages.coggiebot}/bin/coggiebot $out/coggiebot
              ln -s ${packages.coggiebotd-bootstrap-activate}/bin/coggiebot $out/activate
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
