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

  outputs = { self, nixpkgs, flake-utils, naersk, crane }:
    rec {
      inherit (flake-utils.lib.eachDefaultSystem (system:
        let
          installDir = "/var/coggiebot";
          pkgs = import nixpkgs { inherit system; };
          stdenv = pkgs.stdenv;
          naerk-lib = pkgs.callPackage naersk { };
          recursiveMerge = pkgs.callPackage ./iac/lib.nix {};
          cogpkgs = pkgs.callPackage ./iac/coggiebot/default.nix { inherit naerk-lib self recursiveMerge; };
          vanilla-linux = pkgs.callPackages ./iac/vanilla-linux/default.nix {};

        in rec {
          inherit cogpkgs;
          packages.coggiebot-stable = cogpkgs.mkCoggiebot {
            features-list = with cogpkgs.features; [
              basic-cmds
              bookmark
              mockingbird
            ];
          };

          packages.coggiebot-prerelease = cogpkgs.mkCoggiebot {
            features-list = with cogpkgs.features; [
              mockingbird
            ];
          };

          # Deployment environment for normal linux machines.
          packages.deploy = vanilla-linux.deploy {
            inherit installDir;
            coggiebot = packages.coggiebot;
          };

          packages.default = packages.coggiebot-stable;
          hydraJobs = packages.coggiebot;
          devShell =
            pkgs.mkShell packages.canary;

        }))
        packages devShell;

      nixosModules.coggiebot = {pkgs, lib, config, coggiebot}:
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
              serviceConfig.ExecStart = "${pkgs.coggiebot}/bin/coggiebot";
              serviceConfig.Restart = "on-failure";
            };
          };
        };
    };
}
