{
  self
  , lib
  , pkgs
  , stdenv
  , naerk-lib
  , recursiveMerge
}:
let
  debug = expr: builtins.trace expr expr;
  meta = pkgs.callPackage ./meta.nix { };

  mkCommand = {
    # list of strings
    aliases ? [],
    # string
    doc ? "undocumented",
    # strings
    examples ? [],
    action ? "message",
    reply ? "message",
    config ? {},
    filters ? [],
  }: { inherit action examples doc filters; };

  # these are
  genericFeature = {
    name
    # override function
    , pkg-override ? (c: c)
    , maintainers ? [meta.maintainers.lunarix]
    # list of strings in the features
    , dependencies ? []
    , commands ? []
  }:
    {
      ${name} = {
        featureName = "${name}";
        inherit dependencies pkg-override commands maintainers;
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
          {
            name = "basic-cmds";
            commands = [
              mkCommand {
                aliases = [ "help" ];
                doc = "Displays this help message";
                examples = [ "help" ];
            }];
          }
          {
            name = "pre-release";
            commands = [
              mkCommand {
                aliases = [ "prerelease" ];
                doc = "Displays this help message";
                examples = [ "@botname prerelease <github-uri>" ];
            }];
          }

          { name = "bookmark";
            commands = [(mkCommand {
              config.emote = "\u{1F516}";
              doc = "bookmark a message";
              action = "emote";
            })];
          }
          { name = "list-feature-cmd"; }
          {
            name = "prerelease";
            pkg-override = (prev: {
              prev.buildInputs = prev.buildInputs ++ [ pkgs.git ];
            });
          }
          {
            name = "mockingbird";
            pkg-override = mockingbird-lib.mockingbird-fn;
            commands = map(x: (mkCommand x))
              [
                { aliases = ["queue" "play"];
                  doc = ''
                    uses ytdl's backend to stream audio.
                    https://github.com/ytdl-org/youtube-dl/blob/master/docs/supportedsites.md";
                    '';
                  examples = [ "\@botname play <supported-uri>" ];
                }
                { aliases = ["skip"];
                  doc = "skips the current song"; }
                { aliases = ["pause"];
                  doc = "pauses the current song"; }
                { aliases = ["resume"];
                  doc = "resumes the current song"; }
                { aliases = ["stop"];
                  doc = "stops the current song"; }
                { aliases = ["mute"];
                  doc = "self mutes the bot (discord action)"; }
                { aliases = ["deafen"];
                  doc = "self deafens the bot (discord action)"; }
                { aliases = ["unmute"];
                  doc = "self unmutes the bot (discord action)"; }
                { aliases = ["undeafen"];
                  doc = "self undeafens the bot (discord action)"; }
                { aliases = ["leave"];
                  doc = "leaves the voice channel"; }
                { aliases = ["join"];
                  doc = "joins the voice channel"; }
            ];
          }
          {
            name = "demix";
            pkg-override = mockingbird-lib.demix-fn;
            dependencies = [ "mockingbird" ];
            commands = map(x: (mkCommand x))
              [{
                aliases = ["overlay:queue"];
                doc = "modifies the play/queue commands to use deezer's backend to stream audio";
                examples = [ "@botname <deezer/spotify uri>" ];
                config = {
                  arl = {
                    default = "";
                    type = "string";
                    description = "deezer arl token";
                  };
                };
              }];
          }
          {
            name = "dj-room";
            dependencies = [ "mockingbird" ];
            commands = map(x: (mkCommand x))
              [{
                doc = "setups channels to be uri paste dumps for queueing audio";
                examples = [ "<supported uri>" ];
                filters = [ "single-channel-only" ];
                config = {
                  channels = {
                    default = [];
                    type = lib.types.listOf lib.types.int;
                    description = "channel id";
                  };
                };
              }];
          }
          # { name = "fake"; dependencies = [ "nonexist" ]; }
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

  coggiebot-default-args = features-list: {
    name = "coggiebot";
    nativeBuildInputs = [];
    buildInputs = [];

    REV=(self.rev or "canary");
    src = ../../.;

    passthru = {
      inherit features-list meta;
    };
  };

  # New line separated.
  # The suffix number describes if the feature name was enabled. (1: enabled, 0: disabled)
  # The delimiter ':' is used to separate the feature name from the suffix.
  build-profile =
    (coggiebot-drv: (pkgs.writeTextDir
      "share/coggiebot-profile.json"
      builtins.toJSON (meta // {
        features = (which-features-list coggiebot-drv);
        buildInputs = map (drv: drv.name || drv.pname) coggiebot-drv.buildInputs;
        nativeBuildInputs = map (drv: drv.name || drv.pname) coggiebot-drv.nativeBuildInputs;
      })));
in
rec {
  inherit
    meta
    which-features-list
    all-features-list
    build-profile
    genericFeature
    features
    ;

  raw-mockingbird = builtins.removeAttrs (pkgs.callPackage ./mockingbird.nix { inherit genericFeature naerk-lib ; }) ["override" "overrideDerivation"];

  dependency-check = coggiebot:
    let
      nodeps-filter =  lib.filter (f: f.dependencies != []);
      enabled-filter = lib.filter (f: f.enabled);

      marked-features-list = which-features-list coggiebot;
      marked-features = lib.foldl (s: x: (s // { ${x.featureName} = x;}))
        {}
        marked-features-list;

      map-features = map (x: marked-features.${x});

      deps-on-self = lib.filter (f: lib.lists.any (x: x == f.featureName) f.dependencies) all-features-list;

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

      # check that `f.dependencies` are initialized prior to `f`
      # enabled-features-with-wrong-order =
      #   (lib.foldl' (s: f:
      #     let
      #       contains-all = a: lib.lists.all (x: lib.lists.elem x a);
      #       all-deps-enabled = debug (contains-all s.ok f.dependencies);
      #       missing = recursive-missing (x: !(lib.lists.elem x s.ok)) f;
      #     in
      #       {
      #         ok = s.ok ++ (lib.optional (missing == []) [f.featureName]);
      #         err =
      #           s.err ++ (lib.optional (missing != [])
      #             [ (f // ({ inherit missing; })) ]);
      #       }
      # )
      #     { ok=[]; err=[]; }
      #     (lib.filter (x: x.enabled) (marked-features-list))
      # );
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
          ${lib.concatMapStrings (f: "  ${f.name} missing: ${lib.concatMapStrings (x: "${x.name} , ") (debug f.missing)}\n") enabled-features-with-missing-dependencies}
         ''

      # else if (( debug enabled-features-with-wrong-order.err) != []) then

      #   throw ''
      #     The following features are enabled but have dependencies that are not enabled in the correct order:

      #       ${lib.concatMapStrings (f:
      #         "  ${
      #           lib.concatMapStrings (f: "${(debug f).featureName}, -> ${
      #             lib.concatMapStrings (z: "${z.name}, ") (debug f.missing)
      #             }") f
      #         }\n")
      #         (enabled-features-with-wrong-order.err)
      #        }
      # ''
      else
        coggiebot;

  # Force build to have no default features enabled
  # MkCoggiebot' { } -> naesrk-lib.buildPackage -> mkDerivation
  mkCoggiebot = {
    coggiebot ? coggiebot-default-args,
    features-list ? [],
    options ? {},
  }:
    let
      coggie = coggiebot features-list;

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
            # ( build-profile coggie )

          ];
        };
}
