{ config, lib, pkgs, stdenv, installDir, coggiebot, repo, update-heartbeat ? "15min" }:
let
  systemd = pkgs.callPackage ./systemd.nix { inherit coggiebot installDir repo update-heartbeat; };
in
{
  inherit systemd;

  update = systemd.update;

  deploy = stdenv.mkDerivation rec {
    name = "coggie-deploy";
    phases = "buildPhase";
    nativeBuildInputs = [
      coggiebot
      pkgs.coreutils
      systemd.starter
      systemd.updater
      systemd.systemd-start
      systemd.systemd-stop
      systemd.systemd-restart
      systemd.systemd-check
    ];

    PATH = lib.makeBinPath nativeBuildInputs;
    buildPhase = ''
      mkdir -p $out

      ln -s ${systemd.systemd-check}/bin/systemd-check $out/systemd-check
      ln -s ${systemd.starter}/bin/start $out/start-bin
      ln -s ${systemd.updater}/bin/update $out/update
      ln -s ${coggiebot}/bin/${coggiebot.name} $out/coggiebot
      ln -s ${systemd.systemd-enable}/bin/systemd-enable $out/enable
      ln -s ${systemd.systemd-disable}/bin/systemd-disable $out/disable
      ln -s ${systemd.systemd-start}/bin/systemd-start $out/start
      ln -s ${systemd.systemd-stop}/bin/systemd-stop $out/stop
      ln -s ${systemd.systemd-restart}/bin/systemd-restart $out/restart
      ln -s ${systemd.migrate}/bin/migrate $out/migrate
      '';


  };
}
