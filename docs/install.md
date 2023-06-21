# NixOS installation

## Add to NixOS as flake
```nix
{
  description = "NixOS configuration";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
    coggiebot.url = "github:skarlett/coggie-bot";
  };

  outputs = { self, nixpkgs, coggiebot }:
    let
      system = "x86_64-linux";
    in {
      nixosConfigurations.hostname = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          # ...
          coggiebot.nixosModules.coggiebot
        ];
        
        services.coggiebot.enable = true;
        services.coggiebot.environmentFile = "/etc/coggiebot/env_vars";
      };
  };
}

echo "DISCORD_TOKEN=..." >> /etc/coggiebot/env_vars
```


# Standard Linux installation
## Install Nix (Multi-user mode)
```
# jump to root
sudo su

# install nix-multiuser-mode (as ROOT)
sh <(curl -L https://nixos.org/nix/install) --daemon

# activate nix in current session
# If this file does not exist,
# and the installation completed successfully
# starting another terminal/pty session may activate
# nix aswell
. ~/.nix-profile/etc/profile.d/nix.sh
```
---
This project relies on an experimental feature of nix called flakes.
It must be enabled. The following sippet can be used to enable them.
```
# Add flakes to nix
echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf
```

## Install coggiebot
preliminary steps linux distrobutions outside of nixos 
need the following instructions to be ran.

```
adduser coggiebot
INSTALLDIR=/installation/path
mkdir -p $INSTALLDIR
chown coggiebot $INSTALLDIR
su coggiebot

cd $INSTALLDIR

nix build github:skarlett/coggie-bot
```

# Without systemd
If you wish to run coggiebot without systemd, the following will suffice.
```
nix build github:skarlett/coggie-bot --out-dir /installation/path
```

## With systemd
this step enables self updates from github:skarlett/coggie-bot
forks of this project must ensure that their fork is replaced as the source-repo
for the self update scripts.
```
nix build github:skarlett/coggie-bot#coggiebot-deploy --out-dir /installation/path
. /installation/path/enable # Enable systemd units
. /installation/path/start # start systemd units
```

## Deploy without self-update
not supplied, as there is no apparent demand for such.


# Installation without Nix
While not encouraged, it may be desirable to install coggiebot as a traditional application. 
The feature flags on coggiebot are used to determine the dependencies needed to run the project.
By opting out of Nix, dependency support is entirely based on the system administrator. These options may grow irrevelant over time, 
and there is no guarantee that these options represent the state of features and dependencies as they are for each commit. But as of https://github.com/Skarlett/coggie-bot/commit/b6752be2f9f7805087e048090c554ec34cd8a017
The options declared here are working, and will most likely work for further versions. Using nix allievates this problem entirely from the system administrator, and ourselves from documenting the dependencies.

# 2.0 Operations (Without nix)
### Build & Deploy
```
cargo build --release --features bookmark,basic-cmds
```
runtime environment:
  DISCORD_TOKEN="api key"
  RUST_LOG="warn,error,info"  

## Dependencies
- rustc 1.66 >=
- OpenSSL

### Flags
- mockingbird-core: libopus cmake gcc/clang ffmpeg
- mockingbird-ytdl: mockingbird-core, python3.6 >=, yt-dlp/yt-dl 
- mockingbird-deemix: mockingbird-core, python3.6 >=, deemix 3.6.6 >=
  - runtime environ:
    - `DEEMIX_ARL=char[128]`

- mockingbird-spotify: mockingbird-core, mockingbird-deemix, spotipy
  - runtime environ:
    - `DEEMIX_SPT_ID=char[32]`
    - `DEEMIX_SPT_SECRET=char[32]`
    - `DEEMIX_SPT_CACHE="/tmp/auth-file"`
    - `MKBIRD_PIPE_THRESHOLD="0.8"`

- list-feature-cmd:
  normally in nix, this variable is auto generated. currently there is no toml parser provided for
  generating this list.
  - buildtime environ:
    - `COGGIEBOT_FEATURES='FEATURE_NAME=1,FEATURE_TWO=0'`

example:
```
export COGGIEBOT_FEATURE='list-feature-cmd=1,basic-cmds=1,...'
cargo build --release --features list-feature-cmd,basic-cmds,...

DISCORD_TOKEN="..." DEEMIX_ARL="...." ./target/release/coggiebot
```
