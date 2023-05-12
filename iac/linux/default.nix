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

  systemd = pkgs.callPackage ./systemd.nix (base-imports // { inherit features hasFeature; }) ;

  auto-update = pkgs.callPackage ./auto-update
    (base-imports // {
      inherit buildFromSrc;
      inherit (systemd) coggiebotd;
     }
    );

  helpers = pkgs.callPackage ./helpers.nix {
    inherit (auto-update) coggiebotd-update-timer updater;
    inherit (systemd) coggiebotd;
    inherit features
        auto-update
        hasFeature
    ;
  };
in

  buildEnv {
    name = "coggiebot-deployment";
    paths = [
      coggiebot
      systemd.starter
      systemd.coggiebotd
    ]

    ++ lib.optional
      (hasFeature features.systemd-helpers)
      (builtins.attrValues helpers)

    ++ lib.optional
      (hasFeature features.auto-update)
      (builtins.attrValues auto-update)

    ++ lib.optional
      (hasFeature features.systemd-integration-tests)
      [];
  }
