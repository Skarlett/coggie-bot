# mockingbird

mockingbird is a built-in extension to coggiebot which adds audio playing capabilities to a voice call.
It is possible to query YT, beezer, and spotify.

The backend uses [ISRC](https://wikipedia.com/ISRC) to find a licensed copy to use.

Refer to `crates/coggiebot/Cargo.toml` to find the list of feature-names. Compile Coggiebot with features enabled via `cargo build --release --features mockingbird-ytdl,mockingbird-ctrl`

Using these features requires additional configuration of coggiebot, via environment variables.

Some to note of:
    - `DEEMIX_SPT_ID` is spotify's API client ID
    - `DEEMIX_SPT_SECRET` is spotify's API client secret
    - `DEEMIX_SPT_CACHE` is a filesystem path of spotify's session-cookie file.
    - `DEEMIX_ARL` is beezer's session token.
    - `MKBIRD_PIPE_THRESHOLD` is a floating point number between 1.0 - 0.0 where 1 is 100% of the total bytes in the audio track to buffer before playing. As of writing the default value is "0.8" (version #v1.4.16-ci.2 18c0867cd10c863bb9d1bc2986f653a9ed9dbc26).

These features are built in by default in nix, and can be built with `nix build github:skarlett/coggie-bot#coggiebot-stable`

