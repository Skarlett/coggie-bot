
{ perlPackages }:
with perlPackages;
buildPerlPackage {
  pname = "coggie-cleanup";
  version = "0.1.0";
  outputs = ["out"];

  src = ./.;
  propagatedBuildInputs = [ SetObject ];
  preConfigure = ''
    echo "LIB = ${SetObject.out}/lib" > config.in
  '';

  postInstall = ''
    mkdir -p $out/bin
    cp cleanup-downloads.pl $out/bin/coggie-cleanup-deemix
    chmod +x $out/bin/coggie-cleanup-deemix
  '';

  meta = {
    description = "A script to clean up old files in a directory";
  };

}
