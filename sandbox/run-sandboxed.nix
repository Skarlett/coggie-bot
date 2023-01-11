{ mkDerivation, writeShellScript, ...}:
{
 x86_64-linux = { exec }:
  mkDerivation {
    buildPhase = "buildPhase";
    buildInputs = [];
    builder = writeShellScript "builder.sh"
    ''
      $exec > $out
    '';
  };
}
