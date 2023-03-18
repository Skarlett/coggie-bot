{
  config
  , lib
  , pkgs
  , stdenv
  , coggiebot
  , installDir ? "/opt/coggiebot"
}:
rec {
  coggiebotd = pkgs.stdenv.mkDerivation rec {
    name = "coggiebotd.service";
    phases = "buildPhase";

    builder = pkgs.writeShellScript name ''
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
  };

  coggiebotd-update = pkgs.stdenv.mkDerivation rec {
    name = "coggiebotd-update.service";
    phases = "buildPhase";

    builder = pkgs.writeShellScript name ''
      #!/bin/sh
      mkdir -p $out/etc
      cat >> $out/etc/$name <<EOF
      [Unit]
      Description=Automatically update coggiebotd.
      Wants=bookmark-bot-update.timer

      [Service]
      Type=oneshot
      ExecStart=${updater}/bin/update
      TimeoutStartSec=9999

      [Install]
      WantedBy=multi-user.target
      EOF
      chmod 755 $out/etc/$name
    '';

    nativeBuildInputs = [ pkgs.coreutils updater ];
  };

  coggiebotd-update-timer = pkgs.stdenv.mkDerivation rec {
    name = "coggiebotd-update.timer";
    phases = "buildPhase";

    builder = pkgs.writeShellScript name ''
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
  };

  starter = pkgs.stdenv.mkDerivation rec {
    name = "start";
    phases = "buildPhase";

    builder = pkgs.writeShellScript "builder.sh" ''
      #!/bin/sh
      mkdir -p $out/bin/
      cat >> $out/bin/$name <<EOF
      #!/bin/sh
      . ${installDir}/.env
      ${installDir}/result/coggiebot
      EOF
      chmod +x $out/bin/${name}
      '';

    nativeBuildInputs = [ pkgs.coreutils pkgs.nix ];
    PATH = lib.makeBinPath nativeBuildInputs;
  };

  updater = stdenv.mkDerivation rec {
    name = "update";
    phases = "buildPhase";
    builder = ../../sbin/update-builder.sh;
    nativeBuildInputs = [
      pkgs.coreutils
      pkgs.git
      pkgs.nix
      coggiebot
    ];

    origin_url="https://github.com/Skarlett/coggie-bot.git";
    branch = "master";

  };

  systemd-enable = pkgs.stdenv.mkDerivation rec {
    name = "systemd-enable";
    phases = "buildPhase";

    builder = pkgs.writeShellScript name ''
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
      pkgs.coreutils
      coggiebotd
      coggiebotd-update
      coggiebotd-update-timer
    ];
  };

  systemd-disable = pkgs.stdenv.mkDerivation rec {
    name = "systemd-disable";
    phases = "buildPhase";

    builder = pkgs.writeShellScript name ''
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

    buildInputs = [
      pkgs.coreutils
      coggiebotd
      coggiebotd-update
      coggiebotd-update-timer
    ];
  };

  systemd-restart = pkgs.stdenv.mkDerivation rec {
    name = "systemd-restart";
    phases = "buildPhase";

    builder = pkgs.writeShellScript name ''
      #!/bin/sh
      mkdir -p $out/bin
      cat >> $out/bin/$name <<EOF
      #!/bin/sh
      /bin/systemctl restart ${coggiebotd.name}
      /bin/systemctl restart ${coggiebotd-update-timer.name}
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
      pkgs.coreutils
      coggiebotd
      coggiebotd-update
      coggiebotd-update-timer
    ];
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
      pkgs.coreutils
      coggiebotd
      coggiebotd-update
      coggiebotd-update-timer
    ];
  };
}
