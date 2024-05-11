{
  description = "Open source discord utility bot";
  inputs = {
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs =
    { self, nixpkgs, flake-utils, naersk}:

    rec {
      inherit (flake-utils.lib.eachDefaultSystem (system:
        let
          installDir = "/var/coggiebot";

          pkgs = import nixpkgs { inherit system; };
          lib = pkgs.lib;
          stdenv = pkgs.stdenv;
          naerk-lib = pkgs.callPackage naersk { };
          recursiveMerge = pkgs.callPackage ./iac/lib.nix {};
          
          deemix-stream = pkgs.python3Packages.callPackage ./sbin/deemix-stream {};
          cogpkgs = pkgs.callPackage ./iac/coggiebot/default.nix { inherit naerk-lib self recursiveMerge; inherit deemix-stream; };

          stable-features = (with cogpkgs.features; [
              basic-cmds
              bookmark
              list-feature-cmd
              mockingbird-core
              mockingbird-ctrl
              mockingbird-ytdl
              mockingbird-deemix
              mockingbird-deemix-check
              mockingbird-arl-cmd
              mockingbird-debug
              mockingbird-set-arl-cmd
              mockingbird-http
              mockingbird-radio
          ]);

          coggiebot-stable = cogpkgs.mkCoggiebot {
            features-list = stable-features;
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
          packages.deemix-stream = deemix-stream; 
          packages.coggiebot-softcleanup = pkgs.callPackage ./sbin/cleanup-dl {
            perlPackages = pkgs.perl534Packages;
          };

          packages.coggiebot-deploy = vanilla-linux;
          packages.default = coggiebot-stable;
          packages.coggiebot = coggiebot-stable;

          packages.coggiebot-stable = coggiebot-stable;
          packages.coggiebot-stable-docker = pkgs.callPackage ./iac/coggiebot/docker.nix {
            coggiebot = coggiebot-stable;
          };

          packages.cache-target = coggiebot-stable;
        })) packages;

      nixosModules.coggiebot = {pkgs, lib, config, ...}:
        with lib;
        let cfg = config.services.coggiebot;
        in {
          options.services.coggiebot = {
            enable = mkEnableOption "coggiebot service";
            environmentFile = mkOption {
              type = types.path;
            };

          };

          config = mkIf cfg.enable {
            nix.settings.trusted-public-keys = [
              "coggiebot.cachix.org-1:nQZzOJPdTU0yvMlv3Hy7cTF77bfYS0bbf35dIialf9k="
            ];

            users.users.coggiebot = {
              isSystemUser = true;
              group = "coggiebot";
            };
            users.groups.coggiebot = {};
            systemd.tmpfiles.rules = [
              "d /var/lib/coggiebot 700 coggiebot coggiebot"
            ];
            systemd.services.coggiebot = {
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];
              wants = [ "network-online.target" ];
              serviceConfig.EnvironmentFile = cfg.environmentFile;
              serviceConfig.ExecStart = "${self.outputs.packages."${pkgs.system}".coggiebot-stable}/bin/coggiebot";
              serviceConfig.Restart = "on-failure";
              serviceConfig.User = "coggiebot";
            };
          };
        };

      # nixosModules.self-update = {pkgs, lib, config, ...}:
      #   with lib;
      #   let cfg = config.services.self-update;
      #   in {
      #     options.services.self-update = {
      #       enable = mkEnableOption "self-update service";
      #       flake = mkOption {
      #         type = types.str;
      #         example = "github:skarlett/coggiebot";
      #       };
      #     };

      #     config = mkIf cfg.enable {
      #       systemd.services.self-update = {
      #         wantedBy = [ "multi-user.target" ];
      #         after = [ "network.target" ];
      #         wants = [ "network-online.target" ];
      #         serviceConfig.ExecStart = "nixos-rebuild --flake '${cfg.flake}' switch";
      #         serviceConfig.Restart = "on-failure";
      #       };
      #     };
      #   };
    };
}
