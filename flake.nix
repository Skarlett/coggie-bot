{
  description = "Open source discord utility bot";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-22.11";
    nixpkgs-unstable.url = "nixpkgs/nixos-unstable";

    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    deploy-rs.url = "github:serokell/deploy-rs";
  };

  outputs = { self, nixpkgs, nixpkgs-unstable, flake-utils, naersk, deploy-rs }:
    rec {
      inherit (flake-utils.lib.eachDefaultSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          naerk-lib = pkgs.callPackage naersk { };
        in
        {
          packages.coggiebot = naerk-lib.buildPackage { src = ./.; REV=(self.rev or "canary"); };
          devShell =
            pkgs.mkShell { nativeBuildInputs = with pkgs; [ rustc cargo ]; };

          packages.nixosConfigurations.host =
            let
              overlay-unstable = system: final: prev: {
              unstable = import nixpkgs-unstable {
                inherit system;
                config.allowUnfree = true;
              };
            };

            in nixpkgs.lib.nixosSystem {
              inherit system;
              specialArgs = { inherit self; };
              modules = [
                ({self, config, pkgs, ...}: { nixpkgs.overlays = [ (overlay-unstable system) self.overlays.default ]; })
                ({self, config, pkgs, lib, ...}:
                  {
                    networking.hostName = "coggiebot"; # Define your hostname.
                    documentation.dev.enable = true;

                    boot.loader.grub.enable = true;
                    boot.loader.grub.version = 2;
                    boot.loader.grub.device = "/dev/vda";

                    imports = [
                      self.nixosModules.coggiebot
                    ];

                    # services.coggiebot.enable = true;
                    # services.coggiebot.ci-enable = true;
                    services.openssh.enable = true;
                    services.clamav = {
                      daemon.enable = true;
                      updater.enable = true;
                    };

                    boot.initrd.availableKernelModules = [ "xhci_pci" "ahci" "usb_storage" "usbhid" "sd_mod" ];
                    boot.initrd.kernelModules = [ ];
                    boot.kernelModules = [ "kvm-intel" ];

                    fileSystems."/" =
                      { device = "/dev/vda1";
                        fsType = "ext2";
                      };

                    time.timeZone = "America/Chicago";
                    i18n.defaultLocale = "en_US.utf8";

                    nix = {
                      settings = {
                        experimental-features = [ "nix-command" "flakes" ];
                        auto-optimise-store = true;
                      };
                      gc = {
                        automatic = true;
                        dates = "daily";
                        options = "--delete-older-than 5d";
                      };
                    };

                    users.users.lunarix = {
                      shell = pkgs.fish;
                      isNormalUser = true;
                      description = "admin";
                      extraGroups = [ "networkmanager" "wheel" "audio" ];
                      packages = with pkgs;
                        [];
                    };

                    users.users.coggiebot = {
                      isSystemUser = true;
                      description = "coggiebot manages its own repo with the help of humanoids.";
                      extraGroups = ["libvirtd"];
                      group = "coggiebot";
                      packages = with pkgs;
                        [ ];
                    };

                    security.rtkit.enable = true;
                    security.virtualisation.flushL1DataCache = "always";

                    virtualisation.libvirtd.qemu.runAsRoot = false;
                    virtualisation.libvirtd.enable = true;

                    # This value determines the NixOS release from which the default
                    # settings for stateful data, like file locations and database versions
                    # on your system were taken. It‘s perfectly fine and recommended to leave
                    # this value at the release version of the first install of this system.
                    # Before changing this value read the documentation for this option
                    # (e.g. man configuration.nix or on https://nixos.org/nixos/options.html).
                    system.stateVersion = "22.05"; # Did you read the comment?

                  })
                ];
            };
        }))
        packages devShell nixosConfigurations;

      nixosModules.coggiebot = { pkgs, lib, config, ... }:
        with lib;
        let cfg = config.services.coggiebot;
        in {
          options.services.coggiebot = {
            enable = mkEnableOption "coggiebot service";
            api-key = mkOption {
              type = types.str;
              example = "<api key>";
            };
            enable-ci = mkEnableOption "enable ci";
          };

          config = mkIf cfg.enable {
            systemd = {
              services.coggiebot = {
                wantedBy = [ "multi-user.target" ];
                after = [ "network.target" ];
                wants = [ "network-online.target" ];
                environment.DISCORD_TOKEN = "${cfg.api-key}";
                serviceConfig.ExecStart = pkgs.coggiebot;
                serviceConfig.Restart = "on-failure";
              };

              services.coggiebot-updater = mkIf cfg.ci-enable {
                wantedBy = [ "multi-user.target" ];
                after = [ "network.target" ];
                wants = [ "network-online.target" ];
                script = ''
                  #!/usr/bin/env bash
                  ###################
                  # lazy script

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

                  nixos-rebuild --flake github:skarlet/coggie-bot#host
                  '';

                serviceConfig =
                  {
                    Type = "oneshot";
                    User= "nobody";
                    Restart = "on-failure";
                  };
              };

              timers.coggiebot-updater = mkIf cfg.ci-enable {
                WantedBy = ["target.timers"];
                after = [ "network.target" ];
                wants = [ "network-online.target" ];

                timerConfig = {
                    OnBootSec = "5m";
                    OnUnitActiveSec = "5m";
                    Unit = "coggiebot-update.service";
                };
              };
            };
          };
      };

      overlays.default = final: prev: {
        coggiebot = with final;
          final.callPackage ({ inShell ? false }: packages { });
      };
    };
}
