{
  lib
  , pkgs
  , stdenv
  , naerk-lib
  , genericFeature
}:

let
  deemix-extractor = stdenv.mkDerivation {
    name = "deemix-extractor";
    installCommand = ''
      mkdir -p $out/bin
      cp pipe_demix $out/bin/pipe_demix
      chmod +x $out/bin/pipe_demix
    '';
    pythonPackages = (py: [ py.deemix py.click ]);
  };

  mockingbird-fn = (prev:
    rec {
      buildInputs = with pkgs; prev.buildInputs ++ [
        libopus
        ffmpeg
        youtube-dl
      ];

      nativeBuildInputs = with pkgs; prev.nativeBuildInputs ++ [
        makeWrapper
        cmake
        gnumake
      ];

      # PATH = lib.makeBinPath buildInputs;
      # fixupPhase =
      #   (prev.fixupPhase or "") + ''
      #     wrapProgram $out/bin/coggiebot \
      #         --prefix PATH : ${lib.makeBinPath (buildInputs ++ nativeBuildInputs)}
      #         --prefix LD_LOAD_LIBRARY_PATH : ${lib.makeLibraryPath buildInputs}
      #     '';
    });

  demix-fn = (prev:
     {
      buildInputs = prev.buildInputs ++ [ deemix-extractor ];
      nativeBuildInputs = prev.nativeBuildInputs ++ [pkgs.cmake];

      # fixupPhase = (prev.fixupPhase or "") + ''
      #      wrapProgram $out/bin/coggiebot \
      #        --prefix PATH : ${deemix-extractor}/bin
      #      '';
    });

in

rec {
  inherit deemix-extractor mockingbird-fn demix-fn;
  # mockingbird-standalone = coggiebot: coggiebot.overrideAttrs mockingbird-fn;
}
