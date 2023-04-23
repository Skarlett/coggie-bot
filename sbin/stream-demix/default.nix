{ pkgs, buildPythonApplication, ... }:
buildPythonApplication {
  pname = "stream";
  version = "0.1.0";
  src = ./.;
  propagatedBuildInputs = [ pkgs.python39Packages.deemix ];
  doCheck = false;
}
