let
  bottomPort = 13000;
  topPort = 23000;
in
{
  description = "Open source discord utility bot";
  inputs =
    let
      base = {
        nixpkgs.url = "github:nixos/nixpkgs/nixos-22.11";
        nixpkgs-unstable.url = "nixpkgs/nixos-unstable";
        flake-utils.url = "github:numtide/flake-utils";
        naersk.url = "github:nix-community/naersk";
        deploy-rs.url = "github:serokell/deploy-rs";
        impermanence.url = "github:nix-community/impermanence";
      };

      extras = {};# import ./cogs/extra-flake-inputs.nix;
      conf = import ./conf.nix;
    in
      (base // extras);

  outputs = {self, nixpkgs, nixpkgs-unstable, flake-utils, naersk, deploy-rs, ... }@extras:
    rec {
      inherit (flake-utils.lib.eachDefaultSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          naerk-lib = pkgs.callPackage naersk { };
          maintainers = pkgs.callPackage ./maintainers.nix;
        in
        {
          packages.coggiebot =
            naerk-lib.buildPackage {
              src = ./.;
              REV=(self.rev or "canary");

              # TODO: Find out how to add feature list
              # cargoOptions = ['' --features "" ''];
          };

          devShell =
            pkgs.mkShell { nativeBuildInputs = with pkgs; [ rustc cargo ]; };

          # packages.genblacklist =
          #   # let
          #   #   x = input: map (i: {} ) input;
          #   # in
          #     nativeBuildInputs = [
          #       pkgs.coreutils
          #       pkgs.git
          #     ];

          #     # PATH = lib.makeBinPath nativeBuildInputs;
          #   };

          packages.nixosConfigurations =
            let
              overlay-unstable = final: prev: {
                unstable = import nixpkgs-unstable {
                  inherit system;
                  config.allowUnfree = true;
                };
              };
            in
            {
              host = nixpkgs.lib.nixosSystem
                {
                  inherit system;
                  specialArgs = { inherit self; };
                  modules = [
                    ({self, config, pkgs, ...}: {
                      nixpkgs.overlays = [
                        overlay-unstable
                        self.overlays.default
                      ];
                    })

                    ({self, config, pkgs, lib, discordToken, ...}:
                      {
                        networking.hostName = "coggiebot"; # Define your hostname.

                        boot.loader.grub.enable = true;
                        boot.loader.grub.version = 2;
                        boot.loader.grub.device = "/dev/vda";

                        imports = [
                          self.nixosModules.coggiebot
                        ];

                        networking.nat.enable = true;
                        networking.nat.internalInterfaces = ["ve-+"];
                        networking.nat.externalInterface = "eth0";
                        services.openssh.enable = true;

                        systemd.services.nginx.serviceConfig.ReadWritePaths = [ "/var/spool/nginx/logs/" ];

                        services.lighttpd = {
                          enable = true;
                          extraConfig = ''
                          $HTTP["url"] =~ "*.discord.com/" {
                            proxy.server = ( "" => (( "host" => "127.0.0.1", "port" => 3000 )))
                          }
                          '';
                        };

                        services.clamav = {
                          daemon.enable = true;
                          updater.enable = true;
                        };

                        boot.initrd.availableKernelModules = [ "xhci_pci" "ahci" "usb_storage" "usbhid" "sd_mod" ];
                        boot.initrd.kernelModules = [ ];
                        boot.kernelModules = [ "kvm-intel" "kvm-amd" ];

                        filesystems."/" =
                          { device = "/dev/vda1";
                            fstype = "ext2";
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

                        networking.firewall.extraCommands = [
                          # "iptables -N incoming-ssh"
                          # "iptables -A OUTPUT -p tcp --sport $PORTNUM_1 -g filter_quota_1"
                          # "iptables -A OUTPUT -p tcp --sport $PORTNUM_2 -g filter_quota_2"
                          # "iptables -A INPUT -p tcp --dport $PORTNUM_1 -g filter_quota_1"
                          # "iptables  -A INPUT -p tcp --dport $PORTNUM_2 -g filter_quota_2"

                          # "iptables -A filter_quota_1 -m quota --quota $QUOTA_1 -g chain_where_quota_not_reached"
                          # "iptables -A filter_quota_1 -g chain_where_quota_is_reached"
                          # "iptables -A filter_quota_2 -m quota --quota $QUOTA_2 -g chain_where_quota_not_reached"
                          # "iptables -A filter_quota_2 -g chain_where_quota_is_reached"
                        ];

                        users.users.lunarix = maintainers.lunarix.profile;

                        users.users.coggiebot = {
                          isSystemUser = true;
                          description = "coggiebot manages its own repo with the help of humanoids.";
                          extraGroups = ["libvirtd"];
                          group = "coggiebot";
                          packages = with pkgs;
                            [];
                        };

                        security.rtkit.enable = true;
                        security.virtualisation.flushL1DataCache = "always";
                        virtualisation.libvirtd.enable = true;

                        # libvirtd runs qemu as unprivileged user qemu-libvirtd.
                        # Changing this option to false may cause file permission issues for existing guests.
                        # To fix these, manually change ownership of affected files
                        # in /var/lib/libvirt/qemu to qemu-libvirtd.
                        virtualisation.libvirtd.qemu.runAsRoot = false;

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
            };
        }))
        packages devShell nixosConfigurations;

      nixosModules = {
        coggiebot = { pkgs, lib, config, ... }:
          with lib;
          let cfg = config.services.coggiebot;
          in {
            options.services.coggiebot = {
              enable = mkEnableOption "coggiebot service";
              api-key = mkOption {
                type = types.str;
                example = "<api key>";
              };

              features = {
                ci = mkEnableOption "enable ci";
                quota = mkEnableOption "enable quotas on containers";
              };
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

                services.coggiebot-updater = mkIf cfg.features.ci {
                  wantedBy = [ "multi-user.target" ];
                  after = [ "network.target" ];
                  wants = [ "network-online.target" ];
                  script = ''
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
                      User= "root";
                      Restart = "on-failure";
                    };
                };

                timers.coggiebot-updater = mkIf cfg.features.ci {
                  WantedBy = ["target.timers"];
                  after = [ "network.target" ];
                  wants = [ "network-online.target" ];

                  timerConfig = {
                      OnBootSec = "5m";
                      OnUnitActiveSec = "5m";
                      Unit = "coggiebot-update.service";
                  };
                };

                services.coggiebot-quota = mkIf cfg.features.quota {
                  WantedBy = ["target.timers"];
                  after = [ "network.target" ];
                  wants = [ "network-online.target" ];
                  script = ''

                  '';
                  serviceConfig =
                    {
                      Type = "oneshot";
                      User= "root";
                      Restart = "on-failure";
                    };
                };

                timers.coggiebot-quota = mkIf cfg.features.quota {
                  WantedBy = ["target.timers"];
                  after = [ "network.target" ];
                  wants = [ "network-online.target" ];
                  timerConfig = {
                      OnBootSec = "5s";
                      OnUnitActiveSec = "24h";
                      Unit = "coggiebot-updat.service";
                  };
                };


              };
            };
          };

        cogUnit = {pkgs, config, lib, exec}:
          with lib;
          let cfg = config.services.coggiebot;
          in {
            options.services.coggiebot-container = {
              enable = mkEnableOption "runs cog";
            };

            config =
              let rundir = "/run/coggiebot/cogs/${exec.name}";
              in
                {
                  services."cog-${exec.name}" = mkIf cfg.enable {
                    wantedBy = [ "multi-user.target" ];
                    after = [ "network.target" ];
                    wants = [ "network-online.target" ];
                    serviceConfig = {
                      RuntimeDirectory = rundir;
                      RootDirectory = rundir;

                      ExecStart = "${exec}/bin/${exec.name}";
                      ExecStop = "${exec}/bin/${exec.name}";
                      # Type = "forking";

                      BindReadOnlyPaths = [
                        "/nix/store"
                      ];

                      # This sets up a private /dev/tty
                      PrivateDevices = true;
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
