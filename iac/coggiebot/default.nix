{
  self
  , lib
  , pkgs
  , stdenv
  , naerk-lib
  , recursiveMerge
}:
let
  meta = pkgs.callPackage ./meta.nix { };
  
  # these are
  genericFeature = args@{
    name
    , fname ? null
    # override function
    , pkg-override ? (c: c)
    , maintainers ? [meta.maintainers.lunarix]
    # list of strings in the features
    , dependencies ? []
    , rustFeature ? true
    , commands ? [] }:
    { ${name} = {
        inherit dependencies
          pkg-override
          commands
          maintainers
          rustFeature;
        featureName = "${name}";
        flagName =
          if fname == null then "${name}"
          else fname;
      };
    };

  features =
      recursiveMerge (
        (lib.foldl (s: x: s ++ [(genericFeature x)]) []
        [
          { 
            name = "list-feature-cmd";
            pkg-override = (prev: {
              COGGIEBOT_FEATURES = lib.concatStringsSep "," (map (x: "${x.featureName}=${if x.enabled then "1" else "0"}") prev.passthru.available-features);
            });  
          }
          { name = "basic-cmds"; }
          { name = "bookmark"; }
          { name = "prerelease";
            pkg-override = (prev: {
              runtimeInputs = prev.runtimeInputs ++ [ pkgs.git ];
            });
          }
          { name = "mockingbird";
            fname = "mockingbird-core";
            pkg-override = (prev: {
              runtimeInputs = prev.runtimeInputs ++ (with pkgs; [
                ffmpeg
              ]);
              buildInputs = with pkgs; prev.buildInputs ++ [
                cmake
                gnumake
                pkgconfig
                libopus
              ];
            });
          }
          { name = "mockingbird-hard-cleanfs";
            dependencies = ["mockingbird-playback"];
          }
          { name = "mockingbird-deemix";
            dependencies = [ "mockingbird" ];
            pkg-override = (prev: {   
              runtimeInputs = prev.runtimeInputs ++ [ pkgs.python39Packages.deemix ];
            });
          }
          { name = "mockingbird-channel";
            dependencies = [ "mockingbird" ];
          }
          { name = "mockingbird-ytdl";
            dependencies = [ "mockingbird" ];
            pkg-override = (prev: {
              runtimeInputs = prev.runtimeInputs ++ [ pkgs.yt-dlp ];
            });
          }
          { name = "mockingbird-playback"; }
          { name = "mockingbird-spotify";
            dependencies = [ "mockingbird-deemix" ];
          }
          { name = "mockingbird-mp3";
            dependencies= ["mockingbird"];
          }
        ])
      );

  all-features-list = lib.mapAttrsToList (_: v: v) features;

  which-features-list' = l:
    lib.foldl (s: f:
      s
      ++ [({enabled = lib.lists.any (x: x == f) l;} // f)])
      [] all-features-list;

  # create a list of all features, add a boolean field (enabled) to signify
  # if coggiebot has that feature enable add a new field named `enabled `and set it to
  # 1 for enabled, and 0 for disabled
  which-features-list = coggiebot:
     which-features-list' coggiebot.passthru.features-list;

  coggiebot-default-args = features-list: {
    name = "coggiebot";
    pname = "coggiebot";
    version = "1.4.10";
    nativeBuildInputs = [];
    buildInputs = with pkgs; [ pkgconfig openssl ];
    runtimeInputs = [];

    REV=(self.rev or "canary");
    src = ../../.;
    doCheck = true;

    postInstall = "";
    passthru = {
      inherit features-list meta;
      available-features = which-features-list' features-list;
      hasFeature = feat: builtins.elem feat features-list;
    };
  };
in
rec {
  inherit
    meta
    which-features-list
    all-features-list
    genericFeature
    features;

  dependency-check = coggiebot:
    let
      nodeps-filter =  lib.filter (f: f.dependencies != []);
      enabled-filter = lib.filter (f: f.enabled);

      marked-features-list = which-features-list coggiebot;
      marked-features = lib.foldl (s: x: (s // { ${x.featureName} = x;})) {}
        marked-features-list;
      map-features = map (x: marked-features.${x});
      deps-on-self =
        lib.filter
          (f: lib.lists.any (x: x == f.featureName) f.dependencies)
          all-features-list;

      recursive-missing = filter: f:
        let
          deps = map-features f.dependencies;
          part = lib.partition filter deps;
          missing = (map (x: {
            name = x.featureName;
            missing = part.wrong;
          }
          )) part.wrong
          ++ (lib.concatMap (recursive-missing filter) part.right);
        in
          if (part.wrong != []) then
            map (x: {name = x.featureName; inherit missing; }) part.wrong
          else
            [];

      nonexistent-deps =
        let dependents = nodeps-filter all-features-list;
        in
          lib.filter (f: !lib.all(dep: (builtins.hasAttr dep features)) f.dependencies) dependents;

      enabled-features-with-missing-dependencies =
        let
          enabled-features = nodeps-filter (enabled-filter marked-features-list);
          missing = lib.flatten (lib.concatMap (recursive-missing (x: x.enabled)) enabled-features);
        in
          missing;
    in
      if (nonexistent-deps != []) then
        throw
        ''
          The following features do not exist within the final "features" set:
          ${
            lib.concatMapStrings (f: "  ${f.featureName} ->  ${lib.concatMapStrings (x: "${x}, ") f.dependencies}\n")
            nonexistent-deps
          }
        ''
      else if (deps-on-self != []) then
        throw ''
          The following features depend on themselves:
          ${
            lib.concatMapStrings (f: "  ${f.featureName} ->  ${lib.concatMapStrings (x: "${x}, ") f.dependencies}\n")
            deps-on-self
          }
        ''
      else if ((enabled-features-with-missing-dependencies) != []) then
        throw ''
          The following features are enabled but have missing dependencies:
          ${
            lib.concatMapStrings (f: "  ${f.name} missing: ${lib.concatMapStrings (x: "${x.name} , ") (f.missing)}\n")
            enabled-features-with-missing-dependencies
          }
         ''
      else
        coggiebot;

  # Top level build tool
  ######################
  # MkCoggiebot
  #   +-> apply-features
  #   |  -> buildInputs
  #   |    -> naesrk-lib.buildPackage
  #   |      -> mkDerivation
  #   +-> Generate systemd services
  mkCoggiebot = {
    coggiebot ? coggiebot-default-args,
    features-list ? [],
  }:
    let
      coggie = coggiebot features-list;
      pkg = # Apply features
        (lib.foldl (c: f: c // (f.pkg-override c))
          coggie (coggie.passthru.features-list
          
          # wrap all runtimeInputs into a bash file
          # which sets up the PATH variable
          ++ [{
              dependencies = [];
              featureName = "";
              flagName = "";

              rustFeature = false;
              pkg-override = (prev: {
                postInstall = lib.optional (prev.runtimeInputs != []) prev.postInstall + ''
                    wrapProgram $out/bin/${prev.name} \
                        --prefix PATH : ${lib.makeBinPath prev.runtimeInputs}
                  '';

                  buildinputs = prev.buildInputs ++ prev.runtimeInputs;
                  nativeBuildInputs = prev.nativeBuildInputs ++ [ pkgs.makeWrapper ];
              });
            }
          ])
        );
      in
        (naerk-lib.buildPackage ((dependency-check pkg) // {
          cargoBuildOptions=
            l: l
               ++ ["--no-default-features"]
               ++ (lib.optional (pkg.passthru.features-list != [])
                 ["--features"] ++ [(lib.concatStringsSep ","
                   (lib.foldl (s: x: s ++ [x.flagName]) []
                     (builtins.filter (x: x.rustFeature) pkg.passthru.features-list))
                 )]);
          }));
    }
