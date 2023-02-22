{ lib, pkgs }:
rec {
  deploy = {installDir, coggiebot}:
    let
      systemd = pkgs.callPackage ./systemd.nix { inherit coggiebot; };
      starter = (systemd.starter installDir);
    in
    pkgs.stdenv.mkDerivation rec {
      name = "coggie-deploy";
      phases = "buildPhase";
      nativeBuildInputs = [
        coggiebot
        pkgs.coreutils
        starter
        systemd.updater
        systemd.systemd-start
        systemd.systemd-stop
        systemd.systemd-restart
      ];

      builder = pkgs.writeShellScript "builder.sh" ''
      mkdir -p $out
      ln -s ${starter}/bin/start $out/start-bin
      ln -s ${systemd.updater}/bin/update $out/update
      ln -s ${coggiebot}/bin/coggiebot $out/coggiebot
      ln -s ${systemd.systemd-enable}/bin/systemd-enable $out/enable
      ln -s ${systemd.systemd-disable}/bin/systemd-disable $out/disable
      ln -s ${systemd.systemd-start}/bin/systemd-start $out/start
      ln -s ${systemd.systemd-stop}/bin/systemd-stop $out/stop
      ln -s ${systemd.systemd-restart}/bin/systemd-restart $out/restart
      '';
    };
}
