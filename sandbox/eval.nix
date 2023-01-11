{ self, runCommand}:
{
  x86_64-linux = { exec }:
    runCommand "build" {
      buildInputs = [ exec ];
    } "${exec} > $out";

  jar = { exec, jdk, opts ? [] }:
    runCommand "build" {
      buildInputs = [ exec jdk ];
    } "java ${self.lib.addArgs opts} -jar ${exec} > $out";

  mono = {exec, mono, opts ? []}:
    runCommand "build" {
      buildInputs = [ exec mono ];
    } "mono ${exec} > $out";

  node = {nodejs, src, opts ? []}:
    runCommand "build" {
      buildInputs = [nodejs];
    } "node ${self.lib.addArgs opts} $src > $out";

  perl = {src, perl, opts ? []}:
    runCommand "build" {
        buildInputs = [perl];
      } "perl ${self.lib.addArgs opts} $src > $out";

  nix = {nix, src}:
    runCommand "build" {
      buildInputs = [ nix ];
    } "nix eval $src";

  shell = {shell, src}:
    runCommand "build" {
      buildInputs = [ shell ];
    } "$shell $src > $out";
}
