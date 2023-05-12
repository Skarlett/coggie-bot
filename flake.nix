{
  description = "Open source discord utility bot";
  inputs = {
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, nixpkgs, flake-utils, naersk, crane }:

    rec {
      inherit (flake-utils.lib.eachDefaultSystem (system:
        let
          installDir = "/var/coggiebot";

          pkgs = import nixpkgs { inherit system; };
          lib = pkgs.lib;
          stdenv = pkgs.stdenv;
          naerk-lib = pkgs.callPackage naersk { };
          recursiveMerge = pkgs.callPackage ./iac/lib.nix {};
          cogpkgs = pkgs.callPackage ./iac/coggiebot/default.nix { inherit naerk-lib self recursiveMerge; };

          features = with cogpkgs.features; [
            basic-cmds
            bookmark
            mockingbird
          ];

          coggiebot-stable = cogpkgs.mkCoggiebot {
            features-list = features;
          };

          coggiebot-next = cogpkgs.mkCoggiebot {
            features-list = features ++ (with cogpkgs.features; [
              basic-cmds
              bookmark
              list-feature-cmd
              mockingbird
              mockingbird-ytdl
              mockingbird-deemix
              mockingbird-mp3
              mockingbird-playback
              mockingbird-spotify
              mockingbird-hard-cleanfs
            ]);
          };

          migrate = {}; 

          config = {
            prefixes = [];
            bookmark_emoji = "\u{1F516}";
            dj_room = [ 123456789 ];
            features = (cogpkgs.which-features coggiebot-stable);
          };

          coggiebot-dummy = hash: stdenv.mkDerivation {
              name = "coggiebot";
              phases = "buildPhase";
              buildPhase = ''
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
          non-nixos = (pkgs.callPackage ./iac/linux) { features=cogpkgs.features; };

          vanilla-linux = non-nixos {
            inherit installDir;
            coggiebot = coggiebot-stable;
            repo = {
              name = "coggie-bot";
              owner = "skarlett";
              branch = "master";
              deploy = "deploy";
            };
          };

          # Automatically adds a pre-release if able to
          # beta-features is hard coded with the purpose of
          # each branch specifying the exact features its developing
          coggiebot-pre-release =
            cogpkgs.mkCoggiebot {
              features-list = with cogpkgs.features;
                [ mockingbird ];
            };

          cictl = pkgs.callPackage ./sbin/cachectl {
            inherit installDir non-nixos;
            caches = [
              { cachix = true;
                url = "https://coggiebot.cachix.org";
                uid = "coggiebot";
              }
            ];
            packages = [{
              ns = "coggiebot-stable";
              drv=packages.coggiebot-stable;
            }];
          };
        in
          (if (lib.lists.elem cogpkgs.features.pre-release features)
            then { packages.coggiebot-pre-release = coggiebot-pre-release; }
           else {} //

        rec {
          # packages.deploy-workflow-ci = (deploy-dummy "00000000000000000000000000").deploy;
          # packages.deploy-workflow-ci-stage-2 = (deploy-dummy (self.rev or "canary")).deploy;
          # packages.cictl-cachix-upload = cictl.upload-;

          packages.deemix-stream = pkgs.callPackage ./sbin/deemix-stream {
            inherit (pkgs.python39.pkgs) buildPythonApplication;
          };

          packages.cleanup-downloads = pkgs.callPackage ./sbin/cleanup-dl {
            perlPackages = pkgs.perl534Packages;
          };

          packages.deploy = vanilla-linux;
          packages.default = coggiebot-stable;
          packages.coggiebot-stable = coggiebot-stable;
          packages.coggiebot-next = coggiebot-next;

          packages.coggiebot = coggiebot-stable;
        }))) packages;

      nixosModules.coggiebot = {pkgs, lib, config, ...}:
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
              serviceConfig.ExecStart = "${pkgs.coggiebot-stable}/bin/coggiebot";
              serviceConfig.Restart = "on-failure";
            };
          };
        };

      nixosModules.self-update = {pkgs, lib, config, ...}:
        with lib;
        let cfg = config.services.self-update;
        in {
          options.services.self-update = {
            enable = mkEnableOption "self-update service";
            flake = mkOption {
              type = types.str;
              example = "github:skarlett/coggiebot";
            };
          };

          config = mkIf cfg.enable {
            systemd.services.self-update = {
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];
              wants = [ "network-online.target" ];
              serviceConfig.ExecStart = "nixos-rebuild --flake '${cfg.flake}' switch";
              serviceConfig.Restart = "on-failure";
            };
          };
        };

      # checks = flake-utils.lib.eachDefaultSystem (system:
      #   let
      #     pkgs = import nixpkgs { inherit system; };
      #     tests = pkgs.callPackage (nixpkgs + "/nixos/lib/testing-python.nix") {};
      #   in
      #   {
      #     vmTest = tests.makeTest {
      #       nodes = {
      #         client = { ... }: {
      #           imports = [ ];
      #         };
      #       };
      #       testScript =
      #         ''
      #           start_all()
      #           client.wait_for_unit("multi-user.target")
      #           assert "Hello Nixers" in client.wait_until_succeeds("curl --fail http://localhost:8080/")
      #         '';
      #       };
      #   });
    };
}
