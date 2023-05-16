{
  lib
  , pkgs
  , stdenv
  , coggiebot
  , coggiebotd

  , installDir ? "/var/coggiebot"
  , repo
  , buildFromSrc ? true
  , update-heartbeat ? "15min"
}:

let
  migrate = stdenv.mkDerivation rec {
    name = "migrate";
    phases = "buildPhase";
    pull = "github:Skarlett/coggie-bot/master#deploy";

    buildPhase = ''
      mkdir -p $out/bin/
      cat >> $out/bin/${name} <<EOF
      #!/bin/sh
      PULL="\''${PULL:-${pull}}"
      target="\''${TARGET:-${installDir}/result}";
      
      #TODO: make programmic
      cachix use coggiebot
      
      [[ -e \$target/disable ]] && \$target/disable # < 1.4.7
      [[ -e \$target/bin/systemd-disable ]] && \$target/bin/systemd-disable # >= 1.4.7
      ${pkgs.nix}/bin/nix build --refresh --out-link \$target \$PULL
      
      \$target/bin/systemd-enable
      systemctl daemon-reload
      #\$target/start
      systemctl restart ${coggiebotd.name}
      systemctl start ${localSystemdFiles.coggiebotd-update-timer.name}
      EOF
      chmod +x $out/bin/${name}
    '';
    nativeBuildInputs = [
      pkgs.coreutils
      pkgs.nix
      coggiebotd
      localSystemdFiles.coggiebotd-update-timer
    ];
    PATH = lib.makeBinPath nativeBuildInputs;
  };

  updater = stdenv.mkDerivation rec {
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
      DEPLOY_PKG="\''${DEPLOY_PKG:-${repo.deploy}}"
      BUILD_FROM_SRC="\''${BUILD_FROM_SRC:-${if buildFromSrc then "1" else "0"}}"
      BASEPULL="\''${BASEPULL:-github:\$AUTHOR/\$REPO/\$BRANCH}"
      PULL="\$BASEPULL#\$DEPLOY_PKG"
      URI="\''${URI:-https://github.com/\$AUTHOR/\$REPO.git}"

      if [[ \$1 == "--debug" || \$1 == "-d" ]]; then
        echo "DEBUG ON"
        set -xe
      fi

      nix run \$(BASEPULL)#check-cache
      cached=$?

      if [[ $BUILD_FROM_SRC == 0 && cached == 1 ]]; then
        echo "Building from source disabled. \$(BASEPULL)#check-cache returned with exit status \$cached"
        exit 1
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
        ${migrate}/bin/migrate
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

  localSystemdFiles = pkgs.callPackage ./systemd.nix {
    inherit coggiebot coggiebotd update-heartbeat updater;
  };
in
  { inherit updater migrate; } // localSystemdFiles
