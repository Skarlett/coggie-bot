{ mkDerivation, src, writeShellScript, exec }:
mkDerivation {
  buildInputs = [];
  builder = writeShellScript "builder.sh"
  ''
    $exec > $out
  '';
}
