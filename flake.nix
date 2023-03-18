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
          vanilla-linux = pkgs.callPackages ./iac/vanilla-linux/default.nix {};

          # x = all-features-set:
          #   lib.foldr (s: x: s ++ [])
          #     []
          #     (lib.mapAttrsToList (k: v: v) all-features-set);

          features = with cogpkgs.features; [
            basic-cmds
            bookmark
            mockingbird
          ];

          config = {
            prefixes = [];
            dj_room = [ 123456789 ];
            bookmark_emoji = "\u{1F516}";
            rebuild-time = 1500;
          };

          coggiebot-core = cogpkgs.mkCoggiebot {
            features-list = [];
          };

          coggiebot-stable = cogpkgs.mkCoggiebot {
            features-list = features;
          };

          # Automatically adds a pre-release if able to
          # beta-features is hard coded with the purpose of
          # each branch specifying the exact features its developing
          coggiebot-pre-release =
            cogpkgs.mkCoggiebot {
              features-list = with cogpkgs.features;
                [ mockingbird ];
            };
        in
          (if (lib.lists.elem cogpkgs.features.pre-release features)
            then { packages.coggiebot-pre-release = coggiebot-pre-release; }
           else {}) //

        rec {
          packages.coggiebot-stable = coggiebot-stable;

          # Deployment environment for normal linux machines.
          # packages.deploy = vanilla-linux.deploy
          # {
          #  inherit installDir;
          #   coggiebot = packages.coggiebot-stable;
          # }


          # packages.default = coggiebot-stable;
          # hydraJobs = packages.coggiebot;
          #devShell.default =
          #  pkgs.mkShell coggiebot-pre-release;
        }))
        packages; # devShell;

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
    };
}
