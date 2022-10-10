{ pkgs, coggiebot, nixpkgs, ... }:
{

  #nixpkgs.overlays = [ coggiebot.overlay ];
  # config =
  systemd.services.coggiebot = {
    wantedBy = [ "multi-user.target" ];
    after = [ "network.target" ];
    wants = [ "network-online.target" ];
    serviceConfig.ExecStart = "${coggiebot}/bin/coggiebot";
  };
}


# {config, lib}:
#   let
#     cfg = config.coggiebot.services;
#   in
#     with lib;
# {
#   options.coggiebot.enable = mkEnableOption "coggiebot service";

#   config = mkIf cfg.enable
#     {
#       systemd.services.coggiebot =
#         let pkg = self.packages.${system}.default;
#         in {
#           description = "bookmark bot";
#           wantedBy = [ "multi-user.target" ];
#           wants = [ "networking.target" ];
#           after = [ "networking.target" ];
#           serviceConfig = {
#             Type = "simple";
#             ExecStart = "${pkg.coggiebot}/bin/coggiebot";
#             RestartSec = "30s";
#             Restart = "on-failure";
#           };
#         };
#     };

#   environment.systemPackages = [ pkgs.coggiebot ];
# }