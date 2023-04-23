{ pkgs, python39Packages, buildPythonApplication, ... }:
buildPythonApplication {
  pname = "deemix-stream";
  version = "0.1.0";
  src = ./.;
  propagatedBuildInputs = with python39Packages;
    [ deemix ];
  doCheck = false;
}
