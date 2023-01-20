{ self, pkgs, addArgs }:
{
  native = { exec }:
    pkgs.runCommand "build" {
      runtimeInputs = [ exec ];
    } "${exec} > $out";

  jar = { exec, jdk, opts ? [] }:
    pkgs.runCommand "build" {
      runtimeInputs = [ exec jdk ];
    } "java ${addArgs opts} -jar ${exec} > $out";

  mono = {exec, mono, opts ? []}:
    pkgs.runCommand "build" {
      runtimeInputs = [ exec mono ];
    } "mono ${exec} > $out";

  node = {nodejs, src, opts ? []}:
    pkgs.runCommand "build" {
      runtimeInputs = [nodejs];
    } "node ${addArgs opts} $src > $out";

  perl = {src, perl, opts ? []}:
    pkgs.runCommand "build" {
        runtimeInputs = [perl];
      } "perl ${addArgs opts} $src > $out";

  nix = {nix, src}:
    pkgs.runCommand "build" {
      runtimeInputs = [ nix ];
    } "nix eval $src";

  shell = {shell, src}:
    pkgs.runCommand "build" {
      runtimeInputs = [ shell ];
    } "$shell $src > $out";
}
