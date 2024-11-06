{ lib, buildPythonApplication, deemix, spotipy }:
let
  patched-deemix = deemix.override (p: {
    deezer-py = p.deezer-py.overrideAttrs {
      patches = [ ../../deezer-api.patch ];
    };
  });
in
buildPythonApplication {
  pname = "deemix-stream";
  version = "0.0.5";

  propagatedBuildInputs = [ patched-deemix spotipy ];
  src = lib.cleanSource ./.;
}
