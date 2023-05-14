{
  lib
  , pkgs
  , stdenv
  , hasFeature
  , coggiebotd
  , features
  , auto-update
}:
let
  mkSystemdHelpScript = {cmd, usePath ? false}: stdenv.mkDerivation rec {
    name = "systemd-${cmd}";
    phases = "buildPhase";

    buildPhase = ''
      #!/bin/sh
      mkdir -p $out/bin
      cat >> $out/bin/$name <<EOF
      #!/bin/sh

      systemctl ${cmd} ${
        if usePath then
          "${coggiebotd.outPath}/etc/${coggiebotd.name}"
        else
          "${coggiebotd.name}"
      }

      systemctl ${cmd} ${
        if usePath then
          "${auto-update.coggiebotd-update.outPath}/etc/${auto-update.coggiebotd-update.name}"
        else
          "${auto-update.coggiebotd-update.name}"
      }

      ${
        "systemctl ${cmd} ${
            if usePath then
              "${auto-update.coggiebotd-update-timer.outPath}/etc/${auto-update.coggiebotd-update-timer.name}"
            else
              auto-update.coggiebotd-update-timer.name
          }"
      }
      EOF
      chmod 755 $out/bin/$name
    '';

    nativeBuildInputs = [
      pkgs.coreutils
      coggiebotd
    ]
      ++ (with auto-update; [
        coggiebotd-update
        auto-update.coggiebotd-update-timer
      ]);

    PATH = lib.makeBinPath nativeBuildInputs;
  };
in {
  systemd-restart = mkSystemdHelpScript {cmd="restart";};
  systemd-start = mkSystemdHelpScript {cmd="start";};
  systemd-stop = mkSystemdHelpScript {cmd="stop";};
  systemd-status = mkSystemdHelpScript {cmd="status";};
  systemd-enable = mkSystemdHelpScript {cmd="enable"; usePath=true;};
  systemd-disable = mkSystemdHelpScript {cmd="disable";};
}
