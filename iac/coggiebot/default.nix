{
  self
  , lib
  , pkgs
  , stdenv
  , naerk-lib
  , recursiveMerge
}:
let
  coggiebot-setup = features-list:
    {
      name = "coggiebot";
      nativeBuildInputs = [];
      buildInputs = [];

      REV=(self.rev or "canary");
      src = ../../.;

      passthru = {
        inherit features-list;
      };
    };

  # these are
  genericFeature = {name, pkg-override ? (c: c), dependencies ? [], config-options ? {}}:
    {
      ${name} = {
        featureName = "${name}";

        inherit dependencies pkg-override config-options;
      };
    };

  features =
    let
      raw-mockingbird = pkgs.callPackage ./mockingbird.nix { inherit genericFeature naerk-lib ; };
      mockingbird-lib =
        builtins.removeAttrs raw-mockingbird ["override" "overrideDerivation"];
    in
      recursiveMerge (
        (lib.foldl (s: x: s ++ [(genericFeature x)]) []
        [
          { name = "basic-cmds"; }
          { name = "bookmark"; }
          { name = "list-feature-cmd"; }
          {
            name = "mockingbird";
            pkg-override = mockingbird-lib.mockingbird-fn;
          }
          {
            name = "demix";
            pkg-override = coggiebot: mockingbird-lib.demix-fn coggiebot;
            dependencies = [ "mockingbird" ];
          }
          {
            name = "dj-channel";
            dependencies = [ "demix" ];
          }
        ])
      );

  all-features-list = lib.mapAttrsToList (_: v: v) features;

  # create a list of all features, add a boolean field (enabled) to signify
  # if coggiebot has that feature enabled
  which-features-list = coggiebot:
    lib.foldl (s: f:
      s
      # if the feature is enabled, add a new field and set it to 1
      ++ [({enabled = lib.lists.any (x: x == f) coggiebot.passthru.features-list;} // f)])
      [] all-features-list;

  # New line separated.
  # The suffix number describes if the feature name was enabled. (1: enabled, 0: disabled)
  # The delimiter ':' is used to separate the feature name from the suffix.
  featurelist = coggiebot: pkgs.writeTextDir
    "share/coggiebot-features.list"
    ''
      # This file is automatically generated by coggiebot.
      ################################################################
      # It contains a list of features that were enabled for this build.
      # The suffix number describes if the feature name was enabled. (1: enabled, 0: disabled)
      # The delimiter ':' is used to separate the feature name from the suffix.
      #
      # This file is read by coggiebot to determine which features are enabled.
      ${
        lib.concatMapStrings (feature:
          "${feature.featureName}:${if feature.enabled then "1" else "0"}\n")
           (which-features-list coggiebot)
      }
    '';
in
rec {
  inherit
    which-features-list
    all-features-list
    featurelist
    genericFeature
    features
    coggiebot-setup;

  raw-mockingbird = builtins.removeAttrs (pkgs.callPackage ./mockingbird.nix { inherit genericFeature naerk-lib ; }) ["override" "overrideDerivation"];
  ####
  # coggiebotWrapped = pkgs.writeShellScriptBin "coggiebot" ''
  # #!${pkgs.stdenv.shell}
  # export LD_LIBRARY_PATH=${pkgs.libopus}/lib
  # export PATH=${pkgs.ffmpeg}/bin:${pkgs.youtube-dl}/bin:${mockingbird.deemix-extractor}/bin
  # exec ${coggiebot}/bin/coggiebot $@
  # '';
  ####
  dependency-check = coggiebot:
    let
      marked-features-list = (which-features-list coggiebot);
      marked-features = lib.foldl (s: x: (s // { ${x.featureName} = x;}))
        {}
        (builtins.trace "hi" marked-features-list);

      fn-all-dependencies-exist = feature-set:
        lib.all (f:
          lib.all (dep:
            (builtins.hasAttr dep feature-set) f.dependencies)
          ) builtins.trace (builtins.attrValues feature-set) "hi2";

      nonexistent-deps =
        lib.filter (f: lib.all (x: builtins.hasAttr x features) f.dependencies) all-features-list;

      enabled-features-with-missing-dependencies =
        lib.filter
          (f: lib.lists.any (dep: marked-features.${dep}.enabled) f.dependencies)
        marked-features-list;

      enabled-features-with-wrong-order =
        (lib.foldl' (s: f:
              {
                ok = s.ok ++ lib.optional (lib.list.all s f.dependencies) [f.name];
                err =
                  s.err ++ [{
                    name=f.name;
                    dependencies=f.dependencies;
                    missing=lib.lists.filter (x: !lib.lists.elem x s) f.dependencies;
                  }];
              }
        )
          { ok=[]; err=[]; }
          (builtins.trace (lib.filter (x: !x.enabled) (marked-features-list)) "hi")
        ).err;
    in
      if (nonexistent-deps != []) then
        throw ''
          The following features do not exist within the final "features" set:
          ${lib.concatMapStrings (f: "  ${f.featureName} ->  ${lib.concatMapStrings (x: "${x}, ") f.dependencies}\n") nonexistent-deps}
        ''

      else if (enabled-features-with-missing-dependencies != []) then
        throw ''
          The following features are enabled but have missing dependencies:
          ${lib.concatMapStrings (f: "  ${f.featureName}\n") enabled-features-with-missing-dependencies}
          ''
      else if (enabled-features-with-wrong-order != []) then
        throw ''
          The following features are enabled but have dependencies that are not enabled in the correct order:
          ${lib.concatMapStrings (f: "  ${f.featureName} -> ${lib.concatMapStrings (x: "${x}, ") f.dependencies}\n") enabled-features-with-wrong-order}
          ''
      else
        coggiebot;

  # Force build to have no default features enabled
  # MkCoggiebot' { } -> naesrk-lib.buildPackage -> mkDerivation
  mkCoggiebot = {
    features-list ? [],
    options ? {},
  }:
    let
      coggie = coggiebot-setup features-list;

      pkg =
        lib.foldl (c: f: c // (f.pkg-override c))
          coggie coggie.passthru.features-list;

      drv =
        (naerk-lib.buildPackage ((dependency-check pkg) // {
            cargoBuildOptions=
              l: l
                 ++ ["--no-default-features"]
                 ++ (lib.optional (pkg.passthru.features-list != [])
                   ["--features"] ++ [(lib.concatStringsSep ","
                     (lib.foldl (s: x: s ++ [x.featureName]) [] pkg.passthru.features-list)
                   )]);
          }));
    in
      pkgs.symlinkJoin {
          name = "coggiebot";
          paths = [
            drv
            ( featurelist coggie )

          ];
        };
}
