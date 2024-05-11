{ dockerTools, coggiebot, stdenv, ... }:

dockerTools.buildImage {
  name = "coggiebot";
  tag = coggiebot.version;

  runAsRoot = ''
    #!${stdenv.shell}
    ${dockerTools.shadowSetup}

    groupadd -r coggiebot
    useradd -r -g coggiebot -M coggiebot
  '';

  config = {
    Cmd = [ "${coggiebot}/bin/coggiebot" ];
    WorkingDir = "/var/coggiebot";
  };
}
