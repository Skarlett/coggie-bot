{
  buildEnv
  , lib
  , pkgs
  , stdenv
  , features
}:

{
  installDir
  , coggiebot
  , repo
  , update-heartbeat ? "1hour"
  , buildFromSrc ? true
}:
let
  hasFeature = coggiebot.passthru.hasFeature;
  base-imports = {
    inherit coggiebot
      installDir
      repo
      update-heartbeat;
  };

  systemd = pkgs.callPackage ./systemd.nix (base-imports // {  }) ;

  auto-update = pkgs.callPackage ./auto-update
    (base-imports // {
      inherit buildFromSrc;
      inherit (systemd) coggiebotd;
     }

    );

  helpers = pkgs.callPackage ./helpers.nix {
    inherit (systemd) coggiebotd;
    inherit features
        hasFeature
        auto-update
    ;
  };
in
  buildEnv {
    name = "coggiebot-deployment";
    paths = [
      coggiebot
      systemd.starter
      systemd.coggiebotd
      auto-update.updater
      auto-update.migrate
      helpers.systemd-restart
      helpers.systemd-start
      helpers.systemd-stop
      helpers.systemd-status
      helpers.systemd-enable
      helpers.systemd-disable
    ]
      # ++(builtins.attrValues auto-update);

    # ++ lib.optional
    #   (hasFeature features.systemd-helpers)

    # ++ lib.optional
    #   (hasFeature features.auto-update)

    # ++ lib.optional
    #   [];
      ;
  }
