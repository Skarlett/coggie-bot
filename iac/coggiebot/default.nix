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
  # deemix-stream = pkgs.callPackage ../sbin/deemix-stream {};

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
  genericFeature = args@{
    name,
    fname ? null
    # override function
    , pkg-override ? (c: c)
    , maintainers ? [meta.maintainers.lunarix]
    # list of strings in the features
    , dependencies ? []
    , rustFeature ? true
    , commands ? [] }:
    {

      ${name} = {
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
    let
      raw-mockingbird = pkgs.callPackage ./mockingbird.nix { inherit genericFeature naerk-lib ; };
      mockingbird-lib =
        builtins.removeAttrs raw-mockingbird ["override" "overrideDerivation"];
    in
      recursiveMerge (
        (lib.foldl (s: x: s ++ [(genericFeature x)]) []
        [
          { name = "list-feature-cmd"; }
          { name = "basic-cmds";
            commands = [
              mkCommand {
                aliases = [ "rev" ];
                doc = "display the current revision";
                default = "canary build";
              }];
          }
          { name = "pre-release";
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
          {
            name = "prerelease";
            pkg-override = (prev: {
              prev.buildInputs = prev.buildInputs ++ [ pkgs.git ];
            });
          }
          { name = "mockingbird";
            fname = "mockingbird-core";
            pkg-override =
              (prev:
                rec {
                  buildInputs = with pkgs; prev.buildInputs ++ [
                    libopus
                    ffmpeg
                  ];

                  nativeBuildInputs = with pkgs; prev.nativeBuildInputs ++ [
                    makeWrapper
                    cmake
                    gnumake
                  ];

                  postInstall = prev.postInstall + ''
                    wrapProgram $out/bin/coggiebot \
                      --prefix PATH : ${lib.makeBinPath buildInputs}
                  '';
                });

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
                  doc = "self mutes the bot (discord vc action)"; }
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
                   
          # { name = "auto-update";
          #   rustFeature = false;
          # }
          # { name = "systemd-helpers";
          #   rustFeature = false;
          # }
          # { name = "systemd-integration-tests";
          #   rustFeature = false;
          # }

          {
            name = "mockingbird-deemix";
            pkg-override = (prev: {
              nativeBuildInputs = prev.nativeBuildInputs ++ [pkgs.cmake pkgs.gcc];
            });

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
            name = "mockingbird-deemix";
            pkg-override = (prev: {
              buildInputs = prev.buildInputs ++ [ pkgs.python39Packages.deemix ];
              nativeBuildInputs = prev.nativeBuildInputs ++ [pkgs.cmake pkgs.gcc];
            });

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
            name = "mockingbird-channel";
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

          {
            name = "mockingbird-ytdl";
            dependencies = [ "mockingbird" ];

            pkg-override = (prev: rec {
              buildInputs = prev.buildInputs ++ [ pkgs.yt-dlp ];

              postInstall = prev.postInstall + ''
                wrapProgram $out/bin/coggiebot \
                   --prefix PATH : ${lib.makeBinPath buildInputs}
              '';

            });
          }
          {
            name = "mockingbird-playback";
          }
          {

            name = "mockingbird-spotify";
            dependencies = [ "mockingbird-deemix" ];

          }

          {
            name = "mockingbird-mp3";
            dependencies= ["mockingbird"];
            pkg-override = (prev: rec {
              nativeBuildInputs =
                prev.nativeBuildInputs ++
                (with pkgs; [ pkgconfig openssl ]);
            });
          }
        ])
      );

  all-features-list = lib.mapAttrsToList (_: v: v) features;

  # create a list of all features, add a boolean field (enabled) to signify
  # if coggiebot has that feature enable add a new field named `enabled `and set it to
  # 1 for enabled, and 0 for disabled
  which-features-list = coggiebot:
    lib.foldl (s: f:
      s
      ++ [({enabled = lib.lists.any (x: x == f) coggiebot.passthru.features-list;} // f)])
      [] all-features-list;

  coggiebot-default-args = features-list: {
    name = "coggiebot";
    nativeBuildInputs = [];
    buildInputs = [];

    REV=(self.rev or "canary");
    src = ../../.;
    doCheck = true;

    postInstall = "";
    passthru = {
      inherit features-list meta;
      hasFeature = feat: builtins.elem feat features-list;
    };
  };

  # New line separated.
  # The suffix number describes if the feature name was enabled. (1: enabled, 0: disabled)
  # The delimiter ':' is used to separate the feature name from the suffix.
  # build-profile =
  #   (coggiebot-drv: (pkgs.writeTextDir
  #     "share/coggiebot-profile.json"
  #     builtins.toJSON (meta // {
  #       features = (which-features-list coggiebot-drv);
  #       buildInputs = map (drv: drv.name || drv.pname) coggiebot-drv.buildInputs;
  #       nativeBuildInputs = map (drv: drv.name || drv.pname) coggiebot-drv.nativeBuildInputs;
      # })));
in
rec {
  inherit
    meta
    which-features-list
    all-features-list
    genericFeature
    features;

  raw-mockingbird = builtins.removeAttrs (pkgs.callPackage ./mockingbird.nix { inherit genericFeature naerk-lib ; }) ["override" "overrideDerivation"];

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
            lib.concatMapStrings (f: "  ${f.name} missing: ${lib.concatMapStrings (x: "${x.name} , ") (debug f.missing)}\n")
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
    options ? {},
  }:
    let
      coggie = coggiebot features-list;
      pkg = # Apply features
        lib.foldl (c: f: c // (f.pkg-override c))
          coggie coggie.passthru.features-list;
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
