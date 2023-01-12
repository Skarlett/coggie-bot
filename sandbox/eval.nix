{ self, pkgs}:
{
  native = { exec }:
    pkgs.runCommand "build" {
      runtimeInputs = [ exec ];
    } "${exec} > $out";

  jar = { exec, jdk, opts ? [] }:
    pkgs.runCommand "build" {
      runtimeInputs = [ exec jdk ];
    } "java ${self.lib.addArgs opts} -jar ${exec} > $out";

  mono = {exec, mono, opts ? []}:
    pkgs.runCommand "build" {
      runtimeInputs = [ exec mono ];
    } "mono ${exec} > $out";

  node = {nodejs, src, opts ? []}:
    pkgs.runCommand "build" {
      runtimeInputs = [nodejs];
    } "node ${self.lib.addArgs opts} $src > $out";

  perl = {src, perl, opts ? []}:
    pkgs.runCommand "build" {
        runtimeInputs = [perl];
      } "perl ${self.lib.addArgs opts} $src > $out";

  nix = {nix, src}:
    pkgs.runCommand "build" {
      runtimeInputs = [ nix ];
    } "nix eval $src";

  shell = {shell, src}:
    pkgs.runCommand "build" {
      runtimeInputs = [ shell ];
    } "$shell $src > $out";
}
