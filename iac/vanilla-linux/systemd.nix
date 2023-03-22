{
  config
  , lib
  , pkgs
  , stdenv
  , coggiebot
  , repo
  , installDir ? "/opt/coggiebot"
  , update-heartbeat ? "15min"
}:
rec {
  coggiebotd = stdenv.mkDerivation rec {
    name = "coggiebotd.service";

    phases = "buildPhase";
    buildPhase = ''
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

  coggiebotd-update = stdenv.mkDerivation rec {
    name = "coggiebotd-update.service";

    phases = "buildPhase";
    buildPhase = ''
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
    buildPhase =
      ''
      #!/bin/sh
      mkdir -p $out/etc
      cat >> $out/etc/$name <<EOF
      [Unit]
      Description=automatically run self update checks on coggiebotd

      [Timer]
      OnBootSec=${update-heartbeat}
      OnUnitActiveSec=${update-heartbeat}

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

    buildPhase = ''
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

  migrate = pkgs.stdenv.mkDerivation rec {
    name = "migrate";
    phases = "buildPhase";
    pull = "github:Skarlett/coggie-bot/master";

    buildPhase = ''
      mkdir -p $out/bin/
      cat >> $out/bin/${name} <<EOF
      #!/bin/sh
      PULL="\''${PULL:-${pull}}"
      target="\''${TARGET:-${installDir}/result}";
      [[ -e \$target/disable ]] && \$target/disable
      ${pkgs.nix}/bin/nix build --refresh --out-link \$target \$PULL#deploy
      \$target/enable
      systemctl daemon-reload
      systemctl restart ${coggiebotd.name}
      systemctl start ${coggiebotd-update-timer.name}
      EOF
      chmod +x $out/bin/${name}
    '';
    nativeBuildInputs = [ pkgs.coreutils pkgs.nix coggiebotd-update-timer coggiebotd ];
    PATH = lib.makeBinPath nativeBuildInputs;
  };

  
  updater = stdenv.mkDerivation rec {
    inherit coggiebot;
    name = "update";
    phases = "buildPhase";
    buildPhase = ''
      mkdir -p $out/bin
      cat >> $out/bin/$name <<EOF
      #!/usr/bin/env bash
      ###################
      # lazy script
      AUTHOR="\''${AUTHOR:-${repo.owner}}"
      REPO="\''${REPO:-${repo.name}}"
      BRANCH="\''${BRANCH:-${repo.branch}}"
      URI="\''${URI:-https://github.com/\$AUTHOR/\$REPO.git}"

      if [[ \$1 == "--debug" || \$1 == "-d" ]]; then
        echo "DEBUG ON"
        set -xe
      fi

      LOCKFILE=/tmp/coggiebot.update.lock
      touch \$LOCKFILE
      exec {FD}<>\$LOCKFILE

      if ! flock -x -w 1 \$FD; then
        echo "Failed to obtain a lock"
        echo "Another instance of `basename \$0` is probably running."
        exit 1
      else
        echo "Lock acquired"
      fi

      # Fetch latest commit
      FETCH_DIR=\$(mktemp -d -t "coggie-bot.update.XXXXXXXX")
      pushd \$FETCH_DIR
      git init .
      git remote add origin \$URI
      git fetch origin \$BRANCH
      LHASH=\$(git show -s --pretty='format:%H' origin/\$BRANCH | sort -r | head -n 1)
      popd
      rm -rf \$FETCH_DIR

      # hard coded link into nix store
      CHASH=\$(${coggiebot}/bin/coggiebot --built-from --token "")

      #
      # Dont replace canary (in source build)
      #
      if [[ \$CHASH == "canary" || \$LHASH == "canary" ]]; then
          echo "canary build -- nonapplicable"
          exit 0
      fi

      if [[ "\$CHASH" != "\$LHASH" ]]; then
        echo "start migrating"
        PULL="github:\$AUTHOR/\$REPO/\$BRANCH" . ${migrate}/bin/migrate
        echo "migrating finished"
      fi

      rm -f \$LOCKFILE
      EOF
      chmod +x $out/bin/$name
      '';

    nativeBuildInputs = [
      pkgs.coreutils
      pkgs.git
      coggiebot
      migrate
    ];

    PATH = lib.makeBinPath nativeBuildInputs;
  };

  systemd-enable = pkgs.stdenv.mkDerivation rec {
    name = "systemd-enable";
    phases = "buildPhase";

    buildPhase = ''
      #!/bin/sh
      mkdir -p $out/bin
      cat >> $out/bin/$name <<EOF
      #!/bin/sh
      systemctl enable ${coggiebotd}/etc/${coggiebotd.name}
      systemctl enable ${coggiebotd-update}/etc/${coggiebotd-update.name}
      systemctl enable ${coggiebotd-update-timer}/etc/${coggiebotd-update-timer.name}
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

    buildPhase =  ''
      #!/bin/sh
      mkdir -p $out/bin
      cat >> $out/bin/$name <<EOF
      #!/bin/sh
      systemctl disable ${coggiebotd}/etc/${coggiebotd.name}
      systemctl disable ${coggiebotd-update}/etc/${coggiebotd-update.name}
      systemctl disable ${coggiebotd-update-timer}/etc/${coggiebotd-update-timer.name}
      EOF
      chmod +x $out/bin/$name
    '';

    PATH = lib.makeBinPath nativeBuildInputs;
    nativeBuildInputs = [
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
      systemctl restart ${coggiebotd.name}
      systemctl restart ${coggiebotd-update-timer.name}
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
      systemctl start ${coggiebotd.name}
      systemctl start ${coggiebotd-update-timer.name}
      EOF
      chmod +x $out/bin/$name
    '';

    PATH = lib.makeBinPath nativeBuildInputs;
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
      systemctl stop ${coggiebotd.name}
      systemctl stop ${coggiebotd-update-timer.name}
      EOF
      chmod +x $out/bin/$name
    '';

    PATH = lib.makeBinPath nativeBuildInputs;
    nativeBuildInputs = [
      pkgs.coreutils
      coggiebotd
      coggiebotd-update
      coggiebotd-update-timer
    ];
  };

  systemd-check = pkgs.stdenv.mkDerivation rec {
    name = "systemd-check";
    phases = "buildPhase";

    builder = pkgs.writeShellScript "builder.sh" ''
      #!/bin/sh
      mkdir -p $out/bin
      cat >> $out/bin/$name <<EOF
      #!/bin/sh
      strict=0
      if [[ $1 == "-ci" ]]; them
        strict=1
      fi

      units=(
        ${lib.strings.concatStringsSep " " (map (x: "${x.name}") [
          coggiebotd
          coggiebotd-update
          coggiebotd-update-timer
        ])}
      );

      for unit in "\''${units[@]}"; do
        if [[ "\$(systemctl is-enabled \$unit)" != "enabled" ]]; then
          echo "\$unit is not enabled"
          [[ $strict == 1 ]] && exit 1
        fi
        if [[ "\$(systemctl is-active \$unit)" != "active" ]]; then
          echo "\$unit is not active"
          [[ $strict == 1 ]] && exit 1
        fi
      done

      EOF
      chmod +x $out/bin/$name
    '';

    PATH = lib.makeBinPath nativeBuildInputs;
    nativeBuildInputs = [
      pkgs.coreutils
      coggiebotd
      coggiebotd-update
      coggiebotd-update-timer
    ];
  };
}
