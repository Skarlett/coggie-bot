{
  self
  , lib
  , pkgs
  , naerk-lib
  , recursiveMerge
}:
let
  coggiebot = naerk-lib.buildPackage
    {
      name = "coggiebot";
      src = ../../.;
      REV=(self.rev or "canary");
      features = [];
      cargoBuildOptions = l: l ++ [ "--no-default-features" ];
    };

  no-default-features = coggiebot.overrideAttrs (prev:
      {
        cargoBuildOptions = (prev.cargoBuildOptions or []) ++ [ "--no-default-features" ];
      }
     );

  genericFeature = name: coggiebot.overrideAttrs (prev:
    {
      ${name} = {
        features = prev.features ++ [ name ];
      };
    });

  features =
    (genericFeature "basic-cmds") //
    (genericFeature "bookmark") //
    {
      mockingbird = pkgs.callPackage ./mockingbird.nix { inherit coggiebot; };
    };

  # add a feature to coggiebot variant.
  # Example:
  #  fn coggiebot: coggiebot.override -> coggiebot with feature
  #  add-feature coggiebot features.basic-cmds;
  add-feature = coggiebot: rhs:
      (coggiebot.override.overrideAttrs (prev:
      recursiveMerge [ prev rhs ]));

  # adds features to the coggiebot variant.
  # Example:
  #  fn coggiebot: [coggiebot.override ..] -> coggiebot with feature
  #  enable-features coggiebot [ features.basic-cmds ];
  enable-features =
    coggiebot: features:
      lib.foldl' add-feature coggiebot features;

  coggiebot-default =
    with features;
    (enable-features coggiebot
    [
      basic-cmds
      bookmark-features
      mockingbird.demix
    ]);

  # create a list of features that were enabled
  # for a given coggiebot variant. 
  mark-features = coggiebot: lib.foldl' (s: feature:
    [{ enabled = lib.lists.any (x: x == feature) coggiebot.features; } (builtins.attrNames features)])
    [] coggiebot.features;

  # New line separated.
  # The suffix number describes if the feature name was enabled. (1: enabled, 0: disabled)
  # The delimiter ':' is used to separate the feature name from the suffix.
  featurelist = coggiebot: pkgs.runCommand "features" {
    nativeBuildInputs = [ pkgs.coreutils ];
  } ''
      mkdir -p $out/share
      echo > $out/share/features.txt <<EOF
      ${lib.foldl' (s: x: s + x.name + ":" + (if x.enabled then "1" else "0") + "\n") ""
        (mark-features coggiebot)}
      EOF
    '';

  coggieBuildOpts = coggiebot: coggiebot.overrideAttrs (prev:
    {
      cargoBuildOptions = ["--features" (lib.concatStringsSep "," prev.features)];
    });

  coggiebot-install = coggiebot: pkgs.symlinkJoin {
    name = "coggiebot-install";
    paths = [
      (coggieBuildOpts coggiebot)
      (featurelist coggiebot)
    ];
  };
in
rec {
  inherit features featurelist coggiebot-install coggiebot no-default-features;

  # coggiebotWrapped = pkgs.writeShellScriptBin "coggiebot" ''
  # #!${pkgs.stdenv.shell}
  # export LD_LIBRARY_PATH=${pkgs.libopus}/lib
  # export PATH=${pkgs.ffmpeg}/bin:${pkgs.youtube-dl}/bin:${mockingbird.deemix-extractor}/bin
  # exec ${coggiebot}/bin/coggiebot $@
  # '';

  # Force build to have no default features enabled
  mkCoggiebot' = { enabled-features ? [], ... }@args:
    let pkg =
          (enable-features (no-default-features // args) enabled-features);
    in
      (coggiebot-install pkg);

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
