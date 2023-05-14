{
  config
  , lib
  , pkgs
  , stdenv
  , coggiebot
  , update-heartbeat
  , coggiebotd
  , updater
}:
rec {
  coggiebotd-update = stdenv.mkDerivation rec {
    name = "coggiebotd-update.service";
    phases = "buildPhase";
    buildPhase = ''
      #!/bin/sh
      mkdir -p $out/etc
      cat >> $out/etc/$name <<EOF
      [Unit]
      Description=Automatically update coggiebotd.
      Wants=${coggiebotd-update-timer.name}

      [Service]
      ExecStart=${updater}/bin/update
      TimeoutStartSec=9999

      [Install]
      WantedBy=multi-user.target
      EOF
      chmod 755 $out/etc/$name
    '';

    nativeBuildInputs = [ pkgs.coreutils updater ];
  };

  coggiebotd-update-timer = pkgs.stdenv.mkDerivation rec {
    name = "coggiebotd-update.timer";
    phases = "buildPhase";
    buildPhase =
      ''
      #!/bin/sh
      mkdir -p $out/etc
      cat >> $out/etc/$name <<EOF
      [Unit]
      Description=automatically run self update checks on coggiebotd

      [Timer]
      OnBootSec=${update-heartbeat}
      OnUnitActiveSec=${update-heartbeat}

      [Install]
      WantedBy=timers.target

      EOF
      chmod 755 $out/etc/$name
    '';

    nativeBuildInputs = [ pkgs.coreutils ];
  };
}
