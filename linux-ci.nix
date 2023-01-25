{ config, lib, pkgs, ... }:
let
          install_dir="/var/coggiebot";
          systemd_unit="coggiebotd";
in
{
  updater = pkgs.stdenv.mkDerivation rec {
    inherit systemd_unit install_dir;
    name = "update";
    phases = "buildPhase";
    builder = ./sbin/update-builder.sh;
        nativeBuildInputs = [
          pkgs.coreutils
          pkgs.git
          pkgs.coggiebot
        ];

        coggiebot=pkgs.coggiebot;
        origin_url="https://github.com/Skarlett/coggie-bot.git";
        branch = "master";
        nix = pkgs.nix;
        coggiebotd = coggiebotd.name;
        coggiebotd-update-timer = coggiebotd-update-timer.name;
        PATH = lib.makeBinPath nativeBuildInputs;
      };

  systemd-enable = pkgs.stdenv.mkDerivation rec {
        name = "systemd-enable";
        phases = "buildPhase";

        builder = pkgs.writeShellScript "builder.sh" ''
          #!/bin/sh
          mkdir -p $out/bin
          cat >> $out/bin/$name <<EOF
          #!/bin/sh
          /bin/systemctl enable ${coggiebotd}/etc/${coggiebotd.name}
          /bin/systemctl enable ${coggiebotd-update}/etc/${coggiebotd-update.name}
          /bin/systemctl enable ${coggiebotd-update-timer}/etc/${coggiebotd-update-timer.name}
          EOF
          chmod +x $out/bin/$name
        '';
        nativeBuildInputs = [
          pkgs.coreutils coggiebotd
          coggiebotd-update
          coggiebotd-update-timer
        ];

        PATH = lib.makeBinPath nativeBuildInputs;
      };

  systemd-disable = pkgs.stdenv.mkDerivation rec {
        name = "systemd-disable";
        phases = "buildPhase";

        builder = pkgs.writeShellScript "builder.sh" ''
          #!/bin/sh
          mkdir -p $out/bin
          cat >> $out/bin/$name <<EOF
          #!/bin/sh
          /bin/systemctl disable ${coggiebotd}/etc/${coggiebotd.name}
          /bin/systemctl disable ${coggiebotd-update}/etc/${coggiebotd-update.name}
          /bin/systemctl disable ${coggiebotd-update-timer}/etc/${coggiebotd-update-timer.name}
          EOF
          chmod +x $out/bin/$name
        '';
        nativeBuildInputs = [
          pkgs.coreutils
          coggiebotd
          coggiebotd-update
          coggiebotd-update-timer
        ];

        PATH = lib.makeBinPath nativeBuildInputs;
      };

  systemd-start = pkgs.stdenv.mkDerivation rec {
        name = "systemd-start";
        phases = "buildPhase";

        builder = pkgs.writeShellScript "builder.sh" ''
          #!/bin/sh
          mkdir -p $out/bin
          cat >> $out/bin/$name <<EOF
          #!/bin/sh
          /bin/systemctl start ${coggiebotd.name}
          /bin/systemctl start ${coggiebotd-update-timer.name}
          EOF
          chmod +x $out/bin/$name
        '';
        nativeBuildInputs = [
          pkgs.coreutils coggiebotd
          coggiebotd-update
          coggiebotd-update-timer
        ];

        PATH = lib.makeBinPath nativeBuildInputs;
      };

  systemd-stop = pkgs.stdenv.mkDerivation rec {
        name = "systemd-stop";
        phases = "buildPhase";

        builder = pkgs.writeShellScript "builder.sh" ''
          #!/bin/sh
          mkdir -p $out/bin
          cat >> $out/bin/$name <<EOF
          #!/bin/sh
          /bin/systemctl stop ${coggiebotd.name}
          /bin/systemctl stop ${coggiebotd-update-timer.name}
          EOF
          chmod +x $out/bin/$name
        '';
        nativeBuildInputs = [
          pkgs.coreutils coggiebotd
          coggiebotd-update
          coggiebotd-update-timer
        ];

        PATH = lib.makeBinPath nativeBuildInputs;
      };

  starter = pkgs.stdenv.mkDerivation rec {
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
        PATH = lib.makeBinPath nativeBuildInputs;
      };

  coggiebotd = pkgs.stdenv.mkDerivation rec {
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

          WorkingDirectory=${starter}
          ExecStart=${starter}/bin/start

          [Install]
          WantedBy=multi-user.target

          EOF
          chmod 755 $out/etc/$name
        '';

        nativeBuildInputs = [ pkgs.coreutils starter ];
        PATH = lib.makeBinPath nativeBuildInputs;
      };

  coggiebotd-update = pkgs.stdenv.mkDerivation rec {
        name = "coggiebotd-update.service";
        phases = "buildPhase";

        builder = pkgs.writeShellScript "builder.sh" ''
          #!/bin/sh
          mkdir -p $out/etc
          cat >> $out/etc/$name <<EOF
          [Unit]
          Description=Automatically update coggiebotd.
          Wants=bookmark-bot-update.timer

          [Service]
          Type=oneshot
          ExecStart=${updater}/bin/update

          [Install]
          WantedBy=multi-user.target
          EOF
          chmod 755 $out/etc/$name
        '';

        nativeBuildInputs = [ pkgs.coreutils updater ];
        PATH = lib.makeBinPath nativeBuildInputs;
      };

  coggiebotd-update-timer = pkgs.stdenv.mkDerivation rec {
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
        PATH = lib.makeBinPath nativeBuildInputs;
      };

  deploy = pkgs.stdenv.mkDerivation rec {
        name = "coggie-deploy";
        phases = "buildPhase";
        nativeBuildInputs = [
          pkgs.coreutils
          starter
          updater
          coggiebot
          systemd-start
          systemd-stop
        ];

        builder = pkgs.writeShellScript "builder.sh" ''
        mkdir -p $out
        ln -s ${starter}/bin/start $out/start-bin
        ln -s ${updater}/bin/update $out/update
        ln -s ${coggiebot}/bin/coggiebot $out/coggiebot
        ln -s ${systemd-enable}/bin/systemd-enable $out/enable
        ln -s ${systemd-disable}/bin/systemd-disable $out/disable
        ln -s ${systemd-start}/bin/systemd-start $out/start
        ln -s ${systemd-stop}/bin/systemd-stop $out/stop
        '';

        PATH = lib.makeBinPath nativeBuildInputs;
  };
}
