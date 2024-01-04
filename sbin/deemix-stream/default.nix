{ lib, buildPythonApplication, deemix, spotipy }:
buildPythonApplication {
  pname = "deemix-stream";
  version = "0.0.4";

  propagatedBuildInputs = [ deemix spotipy ];
  src = lib.cleanSource ./.;
}
