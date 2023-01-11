# CoggieBot Developer Design Document

> Objective: The goal of providing documentation is to provide a guide on how to build, commit changes, and test local changes (canary) before pushing them upstream. Furthermore, providing the ideology, reasoning, and process on which updates are accepted by the project maintainer.

## Development Toolchain
- [Rust The programming language](https://doc.rust-lang.org/stable/book/)
- [Serenity Discord API](https://docs.rs/serenity/latest/serenity/)
- [Nix Reference manual](https://nixos.org/manual/nix/unstable/introduction.html)

## Project ideology

The ideology of this project is to create an open source project and system where developers may add additions to it. The project uses the concepts of immutable development environments to provide build-time and runtime dependency guarantees to the application. While tools like `cargo` provide a large repository of rust libraries, `nix` extends this capability further by allowing us to specify libraries, and application dependencies. 

An example of this is youtube-dl, a popular application that provides a CLI application to download media from `youtube.com`. Using `nix`, we may append `buildDependency = [ pkgs.youtube-dl ]`into the `flake.nix` file. This implication then allows `coggiebot` to call the CLI application `youtube-dl` from within its deployed environment.

While `cargo` alone gives us a lot, the usage of nix along-side us allows us to bring in more dependencies - this is what allows us to have an *"open source system"*, rather than just am open source project.

### [Search NixPkgs](https://search.nixos.org/packages?channel=unstable)

## Challenges

While the use of immutable environments has the benefits shown above, it does leave some pain points which will are affirmed out below.

- Secret management is out tree
> Secrets are currently included in the parent directory (`/var/coggiebot`) of the build folder (`/var/coggiebot/result`), and is referenced inside of `start` binary. Secrets are currently carried in the forum of runtime environment variables.


- Persistent mutable data challenge.
> Mutable data is out side of source tree, while it is possible to manage this data, it will be a challenge providing continuous integration to it without providing migration scripts to be included within the source tree on a commit by commit basis. For systems which are inconsistently bumping version upgrades, this may corrupt or break the project. 

For this reason, persistent mutable data is frowned upon. 


# Quick start
- Before running the project, you agree to the terms of the license/
- [Install Nix](https://nixos.org/manual/nix/stable/installation/installing-binary.html) in multi-user mode
- run `. ~/.nix-profile/etc/profile.d/nix.sh`
- enable experimental features in Nix with `echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf`

```nix
nix run github:skarlett/coggie-bot#coggiebot
```

## Developing
- Before developing for the project, you agree to the terms of the license.

- The term "Canary" in the context of this project is to build the project from local changes.
To check build information from the binary, run `coggiebot --token "" --built-from`. 

- First Fork the [repository](https://github.com/skarlett/coggie-bot)

### Canary Build Quick start (Cargo)
**Note: Building from Cargo alone is subject to change and may not work in the future, building the project should use `nix`.**
```sh
git clone https://github.com/user/your_fork
cd coggie-bot
git checkout -b your-feature
cargo run --release -- --token ""
```

### Build with Nix
```
git clone https://github.com/user/your_fork
cd coggie-bot
git checkout -b your-feature
nix build .

./result/coggiebot --token "" 
```


## On target, source build, Continuous Integration
```sh
adduser coggiebot

# required to stop systemd from killing long build times 
echo "DefaultTimeoutStartSec=9999s" >> /etc/systemd/system.conf

mkdir -p /var/coggiebot
chown coggiebot /var/coggiebot
su coggiebot

cd /var/coggiebot
nix build github:skarlett/coggie-bot

/var/coggie/result/enable
/var/coggie/result/start
```

### CI underbelly
While its obvious that Nix helps build the necessary files to run the project, it also provides the systemd unit files. The previous `coggiebot` is called with `--built-from`, which will return the git hash it was built from. This hash is compared with the content of the origin's branch `FETCH_HEAD`, if the hashes differ the `update` script is ran, rebuilds the project, then re-links & restarts all the necessary systemd units.

The hash emitted from `coggiebot` is the result of the environment variable `REV` which is passed in during build-time. `REV=$(cat .git/ref/head) cargo build --release`. The `REV` environment variable is embedded within the nix build procedures.

## Adding Environment variables

Additional environment variables can be applied at build-time by adding them as variables to the `mkDerivation` and `naerk-lib.buildPackage` body inside of the `flake.nix`.

```nix
# Declare in Nix 
packages.coggiebot = naerk-lib.buildPackage {
    src = ./.;
    REV = (self.rev or "canary");
    variable_name = "My fork!";
}
```
```rust
// Use in rust
fn on_message() {
    println!("environment variable: {}", env!("variable_name"))
}
```

## Managing dependencies
works with both `mkDerivation` and `naerk-lib.buildPackage`
add or remove dependencies by using `buildInput`
```
packages.name = naerk-lib.buildPackage {
    buildInput = [ pkgs.ffmpeg ]
    ...
}
```

## Adding shell scripts
works with both `mkDerivation` and `naerk-lib.buildPackage`
```
packages.hello_world = mkDerivation {
    name = "hello world"
    phases = "buildPhase";
    builder = pkgs.writeShellScript "builder.sh"
    ''
    #!/bin/sh
    echo 'hello world'
    '';
}

packages.caller = {
  name="hello_caller";
  buildInputs = [
    packages.hello_world
    pkgs.coreutils
  ];
}
```

## Using language helpers
- [Language helpers](https://nixos.wiki/wiki/Language-specific_package_helpers)

### Vanilla
```nix
packages.name = {
  name="name";
  builder = ''
    cc hello.c -o $out 
  '';
};
```

Nix provides `cc` which is either `clang` or `gcc`. **Nix always** expects an artifact to be built at `$out`


### C/C++ Addition CMake
```
packages.hello_world = mkDerivation {
  name="hello_C";
  nativeBuildInputs = with pkgs; [
    cmake gnumake clang
  ];
  src = ./.
};
```

### JavaScript
See [napalm](https://github.com/nix-community/napalm)

### Python
See [Poetry2nix](https://github.com/nix-community/poetry2nix) (use flakes example)

### Java
See [gradle2nix](https://github.com/tadfisher/gradle2nix)


#### Nix References
- [The language introduction](https://cheat.readthedocs.io/en/latest/nixos/nix_lang.html)
- [The language deep dive](https://medium.com/@MrJamesFisher/nix-by-example-a0063a1a4c55)
- [Building derivations NixPill](https://nixos.org/guides/nix-pills/our-first-derivation.html) Is it recommended to understand chapters 6-8.
- [How To package software in nix](https://unix.stackexchange.com/questions/717168/how-to-package-my-software-in-nix-or-write-my-own-package-derivation-for-nixpkgs)
- [stdenv mkderivation-builder](https://nixos.org/manual/nixpkgs/unstable/#chap-trivial-builders)
- [3rd party mkderivation-builders](https://nixos.wiki/wiki/Language-specific_package_helpers)
- [nix-nomicon (nixpkgs manual)](https://nixos.org/manual/nixpkgs/unstable/)
- [nix cmd reference (package manager manual)](https://nixos.org/manual/nix/unstable/introduction.html)
- [search nixpkgs](https://search.nixos.org/)
- [Nix Flakes](https://nixos.wiki/wiki/Flakes)
