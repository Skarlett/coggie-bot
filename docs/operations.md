# Coggiebot handbook

# 1.0 Operations

Dependencies:
  nix


### Linux Installation
```
adduser -aG coggiebot

mkdir -p /var/coggiebot
echo > /var/coggiebot/.env <<EOF
export DISCORD_TOKEN="..."
export RUST_LOG="warn,error"
EOF
```

### Manual update
```
/var/coggiebot/systemd-disable
nix build --refresh -o /var/coggiebot/result github:skarlett/coggie-bot#deploy`

/var/coggiebot/bin/systemd-enable
/var/coggiebot/bin/systemd-start
```

### Manual rollback
```
/var/coggiebot/systemd-disable

nix build -o /var/coggiebot/result github:skarlett/coggie-bot/<commit-hash>#deploy
```

### Force run update
```
systemctl start coggiebotd-update

# or /var/coggiebot/bin/update
```

## Deploying on >1GB Machines
---
Its the build process which takes up resources. running `nix build` can cause failure on boxes with less than 1GB of RAM. 

To alleviate this, use the public build cache.
```
cachix use coggiebot
nix build github:skarlett/coggie-bot#deploy
```
**This only works on the master branch.**


For forked variations, create an account at cachix, or use deploy.rs 
to push changes over ssh from your local nix store, to a remote server.


# 2.0 Operations (Without nix)
### Build & Deploy
```
cargo build --release --features bookmark,basic-cmds
```

##### Dependencies
- rustc 1.66 >=
- OpenSSL

- mockingbird-core: libopus cmake gcc
- mockingbird-ytdl: mockingbird-core, python3.6 >=, yt-dlp/yt-dl
- mockingbird-deemix: mockingbird-core, python3.6 >=, deemix
- environ:
  - `DEEMIX_ARL=char[128]`
  - `DEEMIX_CACHE="/tmp/folder"`

- mockingbird-spotify: mockingbird-core, mockingbird-deemix, spotipy
- environ:
  - `SPOTIFY_CLIENT_ID=char[32]`
  - `SPOTIFY_CLIENT_SECRET=char[32]`
   

- list-feature-cmd:
  normally in nix, this variable is auto generated. currently there is no toml parser provided for
  generating this list.

- environ:
    - `COGGIEBOT_FEATURES='FEATURE_NAME=1,FEATURE_TWO=0'`

example:
```
export COGGIEBOT_FEATURE='list-feature-cmd=1,basic-cmds=0,...'
cargo build --release --features list-feature-cmd,...
```

##### features
---
- list-feature-cmd
- basic-cmds
- bookmark
- mockingbird-core
- mockingbird-playback
- mockingbird-channel
- mockingbird-ytdl
- mockingbird-mp3
- mockingbird-spotify
- mockingbird-deemix
- mockingbird-hard-cleanfs

**NOTE:** all runtime dependencies must be on the `PATH` variable.

### Deployments (Systemd)
Non-nix deployments are not officially supported, and will have to be maintained by yourself, or a third party.

Using nix, you can generate systemd unit files of the current generation using `nix build coggiebot#deploy` under `etc/`.
