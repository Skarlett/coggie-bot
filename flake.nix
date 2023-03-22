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

          config = {
            prefixes = [];
            bookmark_emoji = "\u{1F516}";
            dj_room = [ 123456789 ];
            features = (cogpkgs.which-features coggiebot-stable);
          };

          vanilla-linux = (pkgs.callPackage ./iac/vanilla-linux/default.nix) {
            inherit installDir;
            coggiebot = coggiebot-stable;
          } ;

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
           else {} //

        rec {
          # packages.systemd = vanilla-linux.systemd;
          packages.default = coggiebot-stable;
          packages.coggiebot-stable = coggiebot-stable;
          packages.deploy = vanilla-linux.deploy;
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
    };
}
