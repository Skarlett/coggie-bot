# Hacking

First enter into the development environment.

```
nix develop .#coggiebot-stable 
```

after entering into the environment, all `buildInputs` are applied to the `PATH` variable.
commands such as `cargo build` will now work as expected.


to ensure all runtime dependencies are present, run 
```
cargo test
```

### Add Commands / Controller
create a new file called `crates/coggiebot/src/controllers/example.rs`

inside the file add
```rs
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};
use serenity::prelude::*;

const GREETING: &'static str = env!("GREETING");

#[group]
#[commands(hello)]
pub struct Commands;

#[command]
async fn hello(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, GREETING).await
}

#[cfg(tests)]
#[test]
/// assert runtime has ffmpeg
fn has_ffmpeg() {
    use std::env::var;
    use std::path::PathBuf;

    let paths = var("PATH").unwrap();
    assert!(paths.split(':').filter(|p| PathBuf::from(p).join("ffmpeg").exists()).count() >= 1);
}
```


inside of `crates/coggiebot/src/controllers/mod.rs` add the following
```rs

#[cfg(feature="example-feature")]
mod example;


#[allow(unused_mut)]
pub fn setup_framework(mut cfg: StandardFramework) -> StandardFramework {
    add_commands!(
        cfg,
        {
            // add here!
            ["example-feature"] => [example::COMMANDS_GROUP]
        }
    );
    cfg
}
```

# Building with cargo
```
export GREETING="hello!"
cargo build --features example-feature
```


# Finalizing in Nix
Ensure the build works, then travel to `iac/coggiebot/default.nix`
add the following to the `features` list attribute
```nix
{ 
    name = "example-feature";
    pkgs-override = (prev: {
      # nativeBuildInputs = prev.nativeBuildInputs ++ [ pkgs.gcc ];
      # buildInputs = prev.buildInputs ++ [ pkgs.ffmpeg ];
      # add environment variables
      GREETING="g'day mate!"
    })
}
```


now inside of `flake.nix`, locate the `mkCoggiebot` function and add the feature.
```nix
{ packages.coggiebot-stable = mkCoggiebot {
    features = with cogpkgs.features;
      [ example-feature ];
  }
}
```



```sh
git add crates/coggiebot/src/controllers/example.rs
nix build .#coggiebot-experiment
```
