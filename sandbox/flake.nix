let
  src=
    ''
    #include <stdio.h>
    int main() { printf("hello world"); return 0; }
    '';
  system = "x86_64-linux";
in
{
  description = "Language runner";
  inputs.nixpkgs.url = github:nixos/nixpkgs/unstable;

  outputs = ({ self, nixpkgs }:
    let
      pkgs = nixpkgs.${system};
      builders = pkgs.callPackage ./builders.nix { };
      runners = pkgs.callPackage ./eval.nix { };

      srcbuilder = pkgs.mkDerivation {
        inherit src;
        phase = "buildPhase";
        builder = pkgs.runCommand ''
          cat > $out <<EOF
          ${src}
          EOF
        '';
      };

      result = runners.x86_64-linux {
        exec=builders.ccompiler { src = srcbuilder; };
      };

    in
      {
        #lib.addArgs = args: pkgs.lib.concatMapStrings (x: " "+x) args;
        packages.${system} =
          {
            default = result;
          };
      });
}
