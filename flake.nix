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

          features = (with cogpkgs.features; [
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

          coggiebot-stable = cogpkgs.mkCoggiebot {
            features-list = features;
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

          cictl = pkgs.callPackage ./sbin/cachectl {
            inherit installDir non-nixos;
            flakeUri = "github:skarlett/coggie-bot";
            caches = [
              { cachix = true;
                url = "https://coggiebot.cachix.org";
                uid = "coggiebot";
              }
            ];
            packages = [{
              ns = "coggiebot-stable";
              drv = coggiebot-stable;
            }];
          };
        in
        rec {
          packages.check-cache = cictl.check;

          packages.deemix-stream = pkgs.callPackage ./sbin/deemix-stream {
            inherit (pkgs.python39.pkgs) buildPythonApplication;
          };

          packages.cleanup-downloads = pkgs.callPackage ./sbin/cleanup-dl {
            perlPackages = pkgs.perl534Packages;
          };

          packages.deploy = vanilla-linux;
          packages.default = coggiebot-stable;
          packages.coggiebot-stable = coggiebot-stable;

          packages.coggiebot = coggiebot-stable;
        })) packages;

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
    };
}
