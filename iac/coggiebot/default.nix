{
  self
  , lib
  , pkgs
  , naerk-lib
  , recursiveMerge
}:
let
  coggiebot = lib.makeOverridable naerk-lib.buildPackage
    {
      src = ../../.;
      REV=(self.rev or "canary");
      features = [];
      cargoBuildOptions = (l: l ++ ["--no-default-features"]);
    };

  features = {
    mockingbird = pkgs.callPackage ./mockingbird.nix { inherit coggiebot; };

    no-default-features = coggiebot.overrideAttrs (prev:
      {
        # cargoBuildOptions = l: (prev.cargoBuildOptions []) ++ [ "--no-default-features" ];
     });

    basic-cmds = coggiebot.overrideAttrs (prev:
      {
        features = (prev.features or []) ++ [ "basic-cmds" ];
      });

    bookmark-features = coggiebot.override.overrideAttrs (prev:
       {
         features = (prev.cargoFeatures or []) ++ [ "bookmark-react"];
       });
  };

  add-feature = coggiebot: rhs:
    (coggiebot.override.overrideAttrs (prev:
      recursiveMerge [ prev rhs ]));

  enable-features =
    coggiebot: features:
      lib.foldl' add-feature coggiebot features;

  # with features; [ basic-cmds bookmark-features ]
  coggiebot-default =
    with features;
    (enable-features coggiebot
    [
      basic-cmds
      bookmark-features
      mockingbird.demix
    ]);

  coggieBuildOpts = pkg: pkg.overrideAttrs (prev:
    {
      cargoBuildFlags = (prev.cargoBuildFlags or []) ++ [ "--features" (lib.concatStringsSep "," pkg.features) ];
    });
in
rec {
  inherit features coggiebot;

  # coggiebotWrapped = pkgs.writeShellScriptBin "coggiebot" ''
  # #!${pkgs.stdenv.shell}
  # export LD_LIBRARY_PATH=${pkgs.libopus}/lib
  # export PATH=${pkgs.ffmpeg}/bin:${pkgs.youtube-dl}/bin:${mockingbird.deemix-extractor}/bin
  # exec ${coggiebot}/bin/coggiebot $@
  # '';

  # Force build to have no default features enabled
  mkCoggiebot' = { enabled-features ? [], ... }@args:
    let pkg =
          (enable-features (features.no-default-features // args) enabled-features);

          # (enable-features (coggiebot // args) enabled-features);
    in
      (coggieBuildOpts pkg);

  # build coggiebot with default features
  mkCoggiebot = { coggiebot ? coggiebot-default, features ? [], ... }@args:
    mkCoggiebot' ({
      inherit coggiebot features;
    } // args);


  ## Example usage:
  # feature-name = mkFeature (p: {
  #   buildInputs = p.buildInputs ++ []
  # })}
  mkFeature = f:
    coggiebot.override.overrideAttrs f;
}
