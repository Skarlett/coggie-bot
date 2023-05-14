{
  lib
  , pkgs
  , stdenv
  , coggiebot
  , repo
  , update-heartbeat
  , installDir ? "/var/coggiebot"
  , buildFromSrc ? true
}:

let
  imports = { inherit coggiebot repo update-heartbeat installDir buildFromSrc; };
  auto-update = (pkgs.callPackage ./auto-update imports);
in
rec {
  starter = stdenv.mkDerivation rec {
    name = "start";
    phases = "buildPhase";
    buildPhase = ''
      #!/bin/sh
      mkdir -p $out/bin/
      cat >> $out/bin/$name <<EOF
      #!/bin/sh
      . ${installDir}/.env
      # TODO: investigate why this breaks stuff
      # probably receiving hash before overrides
      # are applied
      ##############
      # ${coggiebot}/bin/coggiebot
      ${installDir}/result/bin/coggiebot
      EOF
      chmod +x $out/bin/${name}
    '';
    nativeBuildInputs = [ pkgs.coreutils pkgs.nix ];
    PATH = lib.makeBinPath nativeBuildInputs;
  };

  coggiebotd = stdenv.mkDerivation rec {
    name = "coggiebotd.service";
    phases = "buildPhase";
    buildPhase = ''
      #!/bin/sh
      mkdir -p $out/etc/systemd/system/
      cat >> $out/etc/$name <<EOF
      [Unit]
      Description=Coggie bot
      Documentation=

      Wants=network.target
      After=network.target

      [Service]
      User=coggiebot
      Group=coggiebot
      SuccessExitStatus=0 1
      Nice=-8

      PrivateDevices=true
      NoNewPrivileges=true
      PrivateTmp=true

      WorkingDirectory=${starter}
      ExecStart=${starter}/bin/start

      [Install]
      WantedBy=multi-user.target

      EOF
      chmod 755 $out/etc/$name
    '';

    nativeBuildInputs = [ pkgs.coreutils starter ];
  };
}
