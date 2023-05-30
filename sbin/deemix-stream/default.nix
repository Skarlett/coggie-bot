{ python39Packages }:
with python39Packages;
buildPythonApplication {
  pname = "deemix-stream";
  version = "0.0.3";

  propagatedBuildInputs = [ deemix ];
  src = ./.;
}
