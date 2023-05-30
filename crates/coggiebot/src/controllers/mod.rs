#[cfg(feature = "bookmark")]
#[path = "bookmark.rs"]
mod bookmark;

#[cfg(feature = "basic-cmds")]
#[path = "basic.rs"]
mod basic;

#[cfg(feature = "list-feature-cmd")]
#[path = "features.rs"]
pub mod features;

#[cfg(feature = "prerelease")]
#[path = "prerelease.rs"]
pub mod prerelease;

use serenity::async_trait;
use serenity::{framework::StandardFramework, client::ClientBuilder};
use serenity::model::{channel::Reaction, gateway::Ready};
use serenity::prelude::*;

macro_rules! add_commands {
    ($framework:expr, { $( [ $($feature:literal),* ] => [ $($group:expr),* ]),* })
        => {
            $(#[cfg(all( $(feature = $feature),* ))]
              { $framework = $framework$(.group(&$group))*; })*
        }
}

#[allow(unused_mut)]
pub fn setup_framework(mut cfg: StandardFramework) -> StandardFramework {
    add_commands!(
        cfg,
        {
            ["basic-cmds"] => [basic::COMMANDS_GROUP],
            ["prerelease"] => [features::PRERELEASE_GROUP::PRERELEASE_GROUP],
            ["list-feature-cmd"] => [features::FEATURES_GROUP],
            ["help-cmd"] => [features::HELP_GROUP],
            ["mockingbird-core"] => [mockingbird::COMMANDS]
        }
    );
    cfg
}

#[allow(unused_mut)]
pub async fn setup_state(mut cfg: ClientBuilder) -> ClientBuilder {
    #[cfg(feature = "mockingbird-core")]
    {
        use mockingbird::init as mockingbird_init;
        cfg = mockingbird_init(cfg).await;
    }
    cfg
}

pub struct EvHandler;

#[async_trait]
impl EventHandler for EvHandler {

    #[allow(unused_variables)]
    async fn reaction_add(&self, ctx: Context, ev: Reaction) {
        #[cfg(feature="bookmark")]
        tokio::spawn(async move {
            use bookmark::bookmark_on_react_add;
            match bookmark_on_react_add(&ctx, &ev).await {
                Ok(_) => {},
                Err(e) => { ev.channel_id.say(&ctx.http, format!("Error: {}", e)).await.unwrap(); },
            };
        });
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}
