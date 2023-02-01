{ config, lib, pkgs, maintainers, conf }:
let
  allowedPorts = port: port >= conf.cog.topPort && conf.cog.topPort >= port;
  allowedPortsMap = portforward: allowedPorts portforward.containerPort && allowedPorts portforward.hostPort;

  mkCogConfig = {
    container
    , maintainer
    , exec
  }: ({
    boot.isContainer = true;

    services.openssh = {
      passwordAuthentication = false;
      kbdInteractiveAuthentication = false;
      permitRootLogin = "no";
    };

    users.users = (maintainers.adminUsers //
      {
        ${maintainer.name} = (maintainer.profile // {
          sshKeys = maintainer.profile ++ maintainers.adminKeys;
        });
      }
    );

    environment.systemPackages = [
      pkgs.stevenblack-blocklist
      pkgs.htop
      pkgs.git
    ] ++ container.environment.systemPackages;
  } // container);

in
rec {
  portAssert = {lib, ...}: forwardPorts:
    let msg =
      "forwardPorts.containerPort or forwardPorts.hostPort
       must be in the range of ${conf.cog.bottomPort} and ${conf.cog.topPort}";
    in
    if (builtins.assertMsg
      (builtins.all allowedPortsMap forwardPorts) msg)
    then forwardPorts
    else builtins.throw "error";

  mkCog = {OsConfig, forwardPorts ? [], vmConfig ? {}}:
    {
      ephemeral = true;
      tmpfs = true;
      # drop root privs
      extraFlags = [ "-U" ];
      forwardPorts = portAssert forwardPorts;
      privateNetwork = true;
      # hostAddress = hostAddr;
      # localAddress = localAddr;
      config =
        mkCogConfig OsConfig;
    };
}
