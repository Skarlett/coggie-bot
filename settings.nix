{ config, lib, pkgs, ... }:
{

  coggiebot = {
    ci.enable = false;
    cog.containers = false;
    cog.libvirt = false;
    cog.deploy = false;
    cog.bottomPort = 9000;
    cog.topPort = 14000;
    features = ["bookmark"];
  };

}
