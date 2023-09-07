{ lib, buildPythonApplication, deemix }:
buildPythonApplication {
  pname = "deemix-stream";
  version = "0.0.4";

  propagatedBuildInputs = [ deemix ];
  src = lib.cleanSource ./.;
}
