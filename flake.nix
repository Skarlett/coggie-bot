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
          systemd_unit="coggiebotd";
        in rec {
          packages.coggiebot = naerk-lib.buildPackage { src = ./.; REV=(self.rev or "canary"); };
          packages.updater = pkgs.stdenv.mkDerivation rec {
            inherit systemd_unit install_dir;
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
            branch = "cd-dev";
            nix = pkgs.nix;
            coggiebotd = packages.coggiebotd;
            coggiebotd-update-timer = packages.coggiebotd-update-timer;
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.systemd-enable = pkgs.stdenv.mkDerivation rec {
            name = "systemd-enable";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              mkdir -p $out/bin
              cat >> $out/bin/$name <<EOF
              #!/bin/sh
              /bin/systemctl enable ${packages.coggiebotd}/etc/${packages.coggiebotd.name}
              /bin/systemctl enable ${packages.coggiebotd-update}/etc/${packages.coggiebotd-update.name}
              /bin/systemctl enable ${packages.coggiebotd-update-timer}/etc/${packages.coggiebotd-update-timer.name}
              EOF
              chmod +x $out/bin/$name
            '';
            nativeBuildInputs = [
              pkgs.coreutils packages.coggiebotd
              packages.coggiebotd-update
              packages.coggiebotd-update-timer
            ];

            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.systemd-disable = pkgs.stdenv.mkDerivation rec {
            name = "systemd-disable";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              mkdir -p $out/bin
              cat >> $out/bin/$name <<EOF
              #!/bin/sh
              /bin/systemctl disable ${packages.coggiebotd}/etc/${packages.coggiebotd.name}
              /bin/systemctl disable ${packages.coggiebotd-update}/etc/${packages.coggiebotd-update.name}
              /bin/systemctl disable ${packages.coggiebotd-update-timer}/etc/${packages.coggiebotd-update-timer.name}
              EOF
              chmod +x $out/bin/$name
            '';
            nativeBuildInputs = [
              pkgs.coreutils
              packages.coggiebotd
              packages.coggiebotd-update
              packages.coggiebotd-update-timer
            ];

            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.systemd-start = pkgs.stdenv.mkDerivation rec {
            name = "systemd-start";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              mkdir -p $out/bin
              cat >> $out/bin/$name <<EOF
              #!/bin/sh
              /bin/systemctl start ${packages.coggiebotd.name}
              /bin/systemctl start ${packages.coggiebotd-update-timer.name}
              EOF
              chmod +x $out/bin/$name
            '';
            nativeBuildInputs = [
              pkgs.coreutils packages.coggiebotd
              packages.coggiebotd-update
              packages.coggiebotd-update-timer
            ];

            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.systemd-stop = pkgs.stdenv.mkDerivation rec {
            name = "systemd-stop";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              mkdir -p $out/bin
              cat >> $out/bin/$name <<EOF
              #!/bin/sh
              /bin/systemctl stop ${packages.coggiebotd.name}
              /bin/systemctl stop ${packages.coggiebotd-update-timer.name}
              EOF
              chmod +x $out/bin/$name
            '';
            nativeBuildInputs = [
              pkgs.coreutils packages.coggiebotd
              packages.coggiebotd-update
              packages.coggiebotd-update-timer
            ];

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
              . ${install_dir}/.env
              ${install_dir}/result/coggiebot
              EOF
              chmod +x $out/bin/${name}
            '';

            nativeBuildInputs = [ pkgs.coreutils pkgs.nix ];
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.coggiebotd = pkgs.stdenv.mkDerivation rec {
            name = "coggiebotd.service";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              mkdir -p $out/etc/

              cat >> $out/etc/$name <<EOF
              [Unit]
              Description=Coggie bot
              Documentation=

              Wants=network.target
              After=network.target

              [Service]
              User=coggiebot
              Group=coggiebot
              SuccessExitStatus=0 1

              PrivateDevices=true
              NoNewPrivileges=true
              PrivateTmp=true

              WorkingDirectory=${packages.starter}
              ExecStart=${packages.starter}/bin/start

              [Install]
              WantedBy=multi-user.target

              EOF
              chmod 755 $out/etc/$name
            '';

            nativeBuildInputs = [ pkgs.coreutils packages.starter ];

            install_dir="/var/coggiebot";
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.coggiebotd-update = pkgs.stdenv.mkDerivation rec {
            name = "coggiebotd-update.service";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              mkdir -p $out/etc
              cat >> $out/etc/$name <<EOF
              [Unit]
              Description=Automatically update coggiebotd.
              Wants=bookmark-bot-update.timer
              User=root
              group=root

              [Service]
              Type=oneshot
              ExecStart=${packages.updater}/bin/update
              [Install]
              WantedBy=multi-user.target
              EOF
              chmod 755 $out/etc/$name
            '';

            nativeBuildInputs = [ pkgs.coreutils packages.updater ];
            PATH = nixpkgs.lib.makeBinPath nativeBuildInputs;
          };

          packages.coggiebotd-update-timer = pkgs.stdenv.mkDerivation rec {
            name = "coggiebotd-update.timer";
            phases = "buildPhase";

            builder = pkgs.writeShellScript "builder.sh" ''
              #!/bin/sh
              mkdir -p $out/etc
              cat >> $out/etc/$name <<EOF
              [Unit]
              Description=automatically run self update checks on coggiebotd

              [Timer]
              OnBootSec=15min
              OnUnitActiveSec=15min

              [Install]
              WantedBy=timers.target

              EOF
              chmod 755 $out/etc/$name
            '';

            nativeBuildInputs = [ pkgs.coreutils ];
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
              packages.systemd-start
              packages.systemd-stop
            ];

            builder = pkgs.writeShellScript "builder.sh" ''
            mkdir -p $out
            ln -s ${packages.starter}/bin/start $out/start-bin
            ln -s ${packages.updater}/bin/update $out/update
            ln -s ${packages.coggiebot}/bin/coggiebot $out/coggiebot
            ln -s ${packages.systemd-enable}/bin/systemd-enable $out/enable
            ln -s ${packages.systemd-disable}/bin/systemd-disable $out/disable
            ln -s ${packages.systemd-start}/bin/systemd-start $out/start
            ln -s ${packages.systemd-stop}/bin/systemd-stop $out/stop
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
