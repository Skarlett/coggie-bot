{ mkDerivation, cargo-flamegraph, target, buildShellScript,  ... }:
{
  flamegraph = mkDerivation {
    buildInput = [ target cargo-flamegraph];
    builder = buildShellScript "builder.sh"
    ''
    #!/usr/bin/env bash
    flamegraph {deriv}/bin/{target.name} -o $out
    '';
  };
}
