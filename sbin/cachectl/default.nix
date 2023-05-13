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
    let name = "check-cache-2";
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

          CACHES=(${
            lib.strings.concatStringsSep "\n"
              (map (c: c.url) caches)
          })

          PACKAGES=(${
            lib.strings.concatStringsSep "\n"
              (map (p: builtins.substring 11 32 p.drv.outPath) packages)
          })

          for package in \$PACKAGES; do
            found=0
            for cache in \$CACHES; do
              response=\$(${pkgs.curl}/bin/curl \
                  --write-out '%{http_code}\n' -s \
                  \$cache/\$package.narinfo)

              if [[ 400 >= \$response ]]; then
                found=1
                echo "Package \$package found in \$cache"
                break
              fi
            done

            if [[ \$found == 0 ]]; then
              echo "Package \$package not found in any cache"
              exitFlag=1
            fi
          done
          exit \$exitFlag
          EOF
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
}
