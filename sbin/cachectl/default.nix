{
  self
  , lib
  , pkgs
  , stdenv
  , writeShellScriptBin
  , caches
  , packages
  , flakeUri
  , installDir
  , non-nixos
}:

let
  coggiebot-dummy = hash:
    let name = "coggiebot";
    in stdenv.mkDerivation rec {
      inherit name;
      phases = "buildPhase";
      buildInputs = [ pkgs.coreutils ];
      builder = writeShellScriptBin name ''
        mkdir -p $out/bin/
        cat > $out/bin/$name <<EOF
        #!${pkgs.runtimeShell}

        containsElement () {
          local e match="\$1"
          shift
          for e; do
            [[ \$e == \$match ]] && echo 0 && return;
          done
          echo 1 && return;
        }

        if [[ \$(containsElement "--built-from" "\$@") == 0 ]]; then
          echo "${hash}"
          exit 0
        else
          ts=\$(date -d "+5 min" +%s)
          while [[ \$ts > \$(date +%s) ]]; do
            sleep 5
          done
        fi
        EOF
        chmod +x $out/bin/$name
      '';
    };

  dummy-install = attr: non-nixos
  ({
    inherit installDir;
    update-heartbeat = "5sec";
    coggiebot = coggiebot-dummy "0000000000000000000000";
    repo = {
      name = "coggie-bot";
      owner = "skarlett";
      deploy = "deploy-workflow-ci-stage-2";
    };
  } // attr);
in
{
  check =
    let name = "check-cache";
    in stdenv.mkDerivation {
      inherit name;
      phases = "buildPhase";
      buildPhase = ''
          mkdir -p $out/bin
          cat > $out/bin/$name <<EOF
          #!/usr/bin/env bash
          set -euo pipefail
          echo "Checking server caches..."
          exitFlag=0

          CACHES=(${lib.strings.concatStringsSep "\n"
            (map (c: "\"${c.url}\"") caches)
          })

          PACKAGES=(${
            lib.strings.concatStringsSep "\n"
              (map (p: "\"${builtins.substring 11 32 p.drv.outPath}\"") packages)
          })

          for package in \$PACKAGES; do
            found=0
            for cache in \$CACHES; do
              response=\$(${pkgs.curl}/bin/curl --write-out '%{http_code}' -s \$cache/\$package.narinfo)
              if [[ "\$(echo \$response | head -c 3)" -lt "400" ]]; then
                found=1
                echo "Package \$package found in \$cache"
                break
              fi
            done

            if [[ \$found == 0 ]]; then
              echo "Not found: \$package"
              exitFlag=1
            fi
          done
          exit \$exitFlag
          EOF
          chmod +x $out/bin/$name
      '';
    };

  dummy = {
    stage-1 = dummy-install {
      update-heartbeat = "2sec";
      repo.deploy="deploy-workflow-ci-stage-2";
    };

    stage-2 = dummy-install {
      update-heartbeat = "10sec";
      coggiebot = coggiebot-dummy (self.rev or "canary");
    };
  };

  upload-cachix =
    let name = "upload-cachix";
    in stdenv.mkDerivation {
      inherit name;
      builder = writeShellScriptBin name ''
        #!/usr/bin/env bash
        set -euo pipefail
        echo "Uploading packages to server caches..."
        exitFlag=0

        PACKAGES=(${
          lib.strings.concatStringsSep "\n"
            (map (p: p.ns) packages)
        });

        CACHES=(${
          lib.strings.concatStringsSep "\n"
            (map (c: c.uid) lib.filter(c: !c.cachix) caches)
        })

        for pkg in \$PACKAGES; do
          for cachix in \$CACHES; do
            nix build ${flakeUri}#\$pkg
            cachix push result
          done
        done

        exit \$exitFlag
      '';
    };


  ci-test =
    let name = "ci-test";
    in stdenv.mkDerivation {
      inherit name;
      buildInputs = [ pkgs.curl ];
      builder = writeShellScriptBin name ''
        #!/usr/bin/env bash
        set -euo pipefail

        mkdir -p /var/coggiebot

        nix build ${flakeUri}#dummy-stage-1 -o /var/coggiebot/result
        /var/coggiebot/result/bin/update

        echo "Checking migration script..."
        if [[ "\$(git rev-parse HEAD)" == "$(/var/coggiebot/result/bin/coggiebot --built-from -t '\')" ]];
        then
          echo "Test passed: rev-check"
        else
          echo "Test failed: rev-check"
          exit 1
        fi

        echo "Checking rolling back to dummy..."
        nix build ${flakeUri}#dummy-stage-1 -o /var/coggiebot/result

        echo "Testing systemd migration..."

        /var/coggiebot/result/bin/start
        systemctl is-active --quiet coggiebotd && echo "Test passed" || (echo "failed to start" && exit 1)
        start_ts=$(systemctl show coggiebotd.service --property ExecMainExitTimestampMonotonic | cut -f 2 -d '=')
        for((i=0;i<3;i++)); do
          echo "trying version check."

          if [[ \$i == 3 ]]; then
            echo "Test failed: Did not automatically update & migrate"
            exit 1
          fi

          if [[ "\$(git rev-parse HEAD)" == "$(/var/coggiebot/result/bin/coggiebot --built-from -t '\')" ]];
          then
            echo "coggiebotd-update: Test passed: rev-check"
            break
          fi
          sleep 5
        done

        now=\$(systemctl show coggiebotd.service --property ExecMainExitTimestampMonotonic | cut -f 2 -d '=')

        if [[ \$start_ts >= \$now  ]]; then
          echo "coggiebot-update: Test failed: Did not automatically update & migrate"
          exit 1
        fi

        systemctl is-active --quiet coggiebotd-update.timer && echo "coggiebotd-update.timer: Test passed" || (echo "coggiebot-update.timer: failed to start" && exit 1)
     '';
    };

}
