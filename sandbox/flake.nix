{
  description = "net runner";
  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-22.11";
  inputs.utils.url = "github:numtide/flake-utils";

  outputs = ({ self, nixpkgs, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        rawcode=
          ''
          #include <stdio.h>

          int main() { printf("hello world"); return 0; }
          '';

        compiler = "ccompiler";
        ext = "c";

        pkgs = import nixpkgs { inherit system; };
        addArgs = args: (pkgs.lib.concatMapStrings (x: " "+x) args);

        srcbuilder = pkgs.writeTextFile {
          name = "source-code.${ext}";
          text = rawcode;
        };

        compilers = pkgs.callPackage ./builder.nix { inherit addArgs; };
        rt = pkgs.callPackage ./eval.nix { inherit addArgs; };

        exec=compilers.ccompiler {
          src=srcbuilder;
          cc=pkgs.gcc;
        };

        runner = rt.native { inherit exec; };
      in
      {
        packages.default = runner;

        packages.list_compilers = pkgs.writeTextFile {
          name = "compiler-list.txt";
          text = (pkgs.lib.concatMapStrings (x: x+"\n") (builtins.attrNames compilers));
        };
      }
    ));
}
