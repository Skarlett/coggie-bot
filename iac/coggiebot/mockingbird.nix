{
  lib
  , pkgs
  , stdenv
  , naerk-lib
  , coggiebot
}:
rec {
  deemix-extractor = stdenv.mkDerivation {
    name = "deemix-extractor";
    installCommand = ''
      mkdir -p $out/bin
      cp pipe_demix $out/bin/pipe_demix
      chmod +x $out/bin/pipe_demix
    '';
    pythonPackages = (py: [ py.deemix py.click ]);
  };

  core =
    let
        buildInputs = with pkgs; [
          libopus
          ffmpeg
          youtube-dl
        ];
        nativeBuildInputs = with pkgs; [
          makeWrapper
          cmake
          gnumake
        ];
    in
      coggiebot.override.overrideAttrs (prev:
        {
          buildInputs = prev.buildInputs ++ buildInputs;
          nativeBuildInputs = prev.nativeBuildInputs ++ nativeBuildInputs;
          features = (prev.cargoFeatures or []) ++ [ "mockingbird" ];
          postInstall =
            (prev.postInstall or "") + ''
              wrapProgram $out/bin/coggiebot \
                 --prefix PATH : ${lib.makeBinPath buildInputs}
              '';
        }
      );

  demix =
      core.override.overrideAttrs (prev:
        {
          buildInputs = prev.buildInputs ++ [ deemix-extractor ];
          nativeBuildInputs = prev.nativeBuildInputs ++ [pkgs.cmake];
          postInstall = (prev.postInstall or "") + ''
            wrapProgram $out/bin/coggiebot \
                --prefix PATH : ${deemix-extractor}/bin
            '';
          features = (prev.cargoFeatures or []) ++ [ "mockingbird" "demix" ];
      });
}
