{
  self,
  pkgs,
  addArgs
}:
{
  ccompiler = { cc, src, inputs ? [], env ? [], opts ? []}:
    pkgs.stdenv.mkDerivation
      rec {
        inherit src;
        name = "c-builder";
        buildInputs = [
          cc
          src
        ];
        PATH = pkgs.lib.makeBinPath buildInputs;
        builder =
        pkgs.writeShellScript "builder.sh"
        ''
        cc $src ${addArgs opts} -o $out;
        '';
      };

  elixir = {
    elixir,
    src,
    inputs ? [],
    opts ? [],
    env ? []
  }:
    pkgs.runCommand "build" (env // {
      buildInputs = [ elixir src ] ++ inputs;
    }) "elixirc ${addArgs opts} $src -o $out";

  rust = {
    elixir,
    src,
    inputs ? [],
    opts ? [],
    env ? []
  }:
    pkgs.runCommand "rustc-srccode-builder" (env // {
      buildInputs = [ elixir src ] ++ inputs;
    }) "elixirc ${addArgs opts} $src -o $out";

  java = {
    src,
    jdk,
    coreutils,
    inputs ? [],
    javac_opts ? [],
      jar_opts ? [],
      env ? {}
  }:
    pkgs.runCommand "build" (env // {
      buildInputs = [src jdk coreutils] ++ inputs;
    })
      ''
      mkdir -p $out/bin
      pushd $out
      javac ${javac_opts} $src
      jar ${addArgs jar_opts} -cf $out/bin/exe $out/main.class
      '';

  nasm = { src, nasm, inputs ? [], filetype ? "elf", opts ? [], env ? {} }:
    pkgs.runCommand "build"
      (env // {
        buildInputs = [ nasm ] ++ inputs;
      } "nasm ${self.lib.addArgs opts} -f ${filetype} $src -o $out");

  python = {python, src, coreutils, inputs ? [], env ? []}:
    pkgs.runCommand "build" (env // {
      buildInputs = [python src coreutils] ++ inputs;
    }) ''
      pushd $out
      version=$(python -c "import sys; print(sys.version_info[0])");
      python -mpy_compile ${src}

      case $version in
            "2") cat *.pyc > $out/bin/exe
            ;;
            "3") cat __pycache__/*.pyc > $out/bin/exe
            ;;
            *) exit 1
      esac
      '';

  ts = {typescript, src, coreutils, opts ? [], inputs ? [], env ? []}:
    pkgs.runCommand "builder.sh"
      (env // {
      buildInputs = [typescript src coreutils] ++ inputs;
      })
      ''
        tsc ${addArgs opts} $src
        cat *.js > $out.js
      '';

  zig = { zig, src, llvm, coreutils, opts ? [], inputs ? [], env ? [],  }:
    pkgs.runCommand "builder.sh"
    (env // {
      buildInputs = [src coreutils zig llvm] ++ inputs;
    })
      ''
            mkdir -p $out
            pushd $out
            zig build-exe ${addArgs opts} $src -o $out
      '';

    # nim masm rust go c#
}
