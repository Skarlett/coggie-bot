{ config, lib, pkgs, maintainers, conf }:
let
  # fn {top=int; bottom=int} => fn int -> bool
  # returns boolean if integer between top and bottom
  inRange = {top, bottom}: port: port >= top && bottom >= port;

  allowedPortsMap =
    let
      cogPortRng = inRange {
        top=conf.cog.topPort;
        bottom=conf.cog.bottomPort;
      };
    in portforward: cogPortRng portforward.containerPort && cogPortRng portforward.hostPort;

  mkCogConfig = {
    container
    , maintainer
    , exec
    , cogUnit
  }: ({
    boot.isContainer = true;

    modules = [
      cogUnit
    ];

    services.cogUnit.enable = true;
    services.cogUnit.target = exec;

    services.openssh = {
      passwordAuthentication = false;
      kbdInteractiveAuthentication = false;
      permitRootLogin = "no";
    };

    users.users = (maintainers.adminUsers //
      {
        ${maintainer.name} = ({
          openssh.authorizedKeys.ssh = maintainer.profile.sshKeys ++ maintainers.adminKeys;
        } // maintainer.profile);
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
  inherit inRange;

  portAssert = {lib, ...}: forwardPorts:
    let msg =
      "forwardPorts.containerPort or forwardPorts.hostPort
       must be in the range of ${conf.cog.bottomPort} and ${conf.cog.topPort}";
    in
    if (builtins.assertMsg
      (builtins.all allowedPortsMap forwardPorts) msg)
    then forwardPorts
    else builtins.throw "error";

  cogCommand = { example ? null, help ? null }:
    { inherit example help; };

  mkCog = {
    OsConfig
    , forwardPorts ? []
    , vmConfig ? {}
    , commands ? {}
    , extraHelp ? "",
  }:
    ({
      ephemeral = true;
      tmpfs = true;

      # drop root privs
      extraFlags = [ "-U" ] ++ (lib.optional (
        if vmConfig ? extraFlags && vmConfig.extraFlags
        then vmConfig.extraFlags
        else []
      ));
      forwardPorts = portAssert forwardPorts;
      privateNetwork = true;
      hostAddress = conf.cog.hostAddr;

      config =
        mkCogConfig OsConfig;

      appdata = {
        inherit commands extraHelp;
      };
    } // vmConfig);

}
