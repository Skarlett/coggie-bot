
{ writeShellScript, mkDerivation, ... }:
{
  ccompiler = {cc, src}:
    mkDerivation
      {
        buildInputs = [
          src
          cc
        ];

        builder = "builder.sh"
          ''
          #!/usr/bin/env bash
          cc $src -o $out
          '';
      };

  elixir = {elixir, src}:
    mkDerivation
      {
        builder = writeShellScript "builder.sh"
          ''
          elixirc $src -o $out
          '';
      };

  java = {jdk, src}:
    mkDerivation
    {
      buildInputs = [jdk];
      builder = writeShellScript "builder.sh"
      ''
      mkdir -p $out/bin
      pushd $out
      javac $src
      jar -cf $out/bin/exe $out/Main.class
      '';
    };

  node = {nodejs, src}:
    mkDerivation
      {
        buildInputs = [nodejs];
        builder = writeShellScript "builder.sh"
        "node $src > $out";
      };

  perl = {src, perl}:
    mkDerivation
      {
        buildInputs = [perl];
        builder = writeShellScript "builder.sh"
          ''
          perl $src
          '';
      };
  nix = {nix, src}:
    mkDerivation
    {
      inputBuild = [ nix ];
      builder = writeShellScript "builder.sh" ''
        nix eval $src
      '';
    };

  nasm = {src, nasm}:
    mkDerivation
    {
      buildInputs = [ nasm ];
      builder = writeShellScript "builder.sh"
        ''
          nasm -f elf $src -o $out
        '';
    };

  python = {python, src}:
    mkDerivation
    {
      builder = writeShellScript "builder.sh" ''
      pushd $out
      version=$(python -c "import sys; print(sys.version_info[0])");
      python -mpy_compile input.txt

      case $version in
            "2") cat *.pyc > $out/bin/exe
            ;;
            "3") cat __pycache__/*.pyc > $out/bin/exe
            ;;
            *) exit 1
      esac

      python $out/bin/exe > output.txt
      '';
    };

  shell = {shell, src}:
    mkDerivation
    {
      inherit shell;
      buildInputs = [ shell ];

      builder = writeShellScript "builder.sh" ''
        $shell $src > $out
      '';
    };

  ts = {typescript, src}:
    mkDerivation
    {
      builder = writeShellScript "builder.sh"
      ''
        tsc $src
        cat *.js > $out/bin/exe
      '';
    };

  zig = {zig, llvm, src}:
    mkDerivation
    {
        buildInputs = [ zig llvm ];
        builder = writeShellScript "builder.sh"
        ''
            mkdir -p $out
            pushd $out
            zig build-exe $src -o $out
        '';
    };

  pascal = {src, fpc}:
    mkDerivation
      {
        buildInputs = [ fpc ];
        builder = writeShellScript "builder.sh"
        ''
        #!/bin/sh
        fpc $src -o $out
        '';
      };

  fortran = {src, gfortran}:
    mkDerivation
    {
      buildInputs = [gfortran];
      builder = ''
      #!/bin/sh
      fortranc $src -o $out
      '';
    };
}
