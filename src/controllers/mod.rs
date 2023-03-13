#[cfg(feature = "bookmark-emoji")]
#[path = "bookmark.rs"]
mod bookmark;

#[cfg(feature = "mockingbird")]
pub mod mockingbird;

#[cfg(feature = "basic-cmds")]
#[path = "basic.rs"]
mod basic;

#[cfg(feature = "basic-cmds")]
#[path = "features.rs"]
pub mod features;

#[cfg(feature = "prerelease")]
#[path = "prerelease.rs"]
pub mod prerelease;

use serenity::model::prelude::Message;
use serenity::{framework::StandardFramework, client::ClientBuilder};
use serenity::async_trait;
use serenity::model::{channel::Reaction, gateway::Ready};
use serenity::prelude::*;

#[allow(unused_mut)]
pub fn setup_framework(mut cfg: StandardFramework) -> StandardFramework {
    #[cfg(feature = "mockingbird")]
    {
        cfg = cfg.group(&mockingbird::MOCKINGBIRD_GROUP);
    }

    #[cfg(all(feature="demix", feature="mockingbird"))]
    {
        cfg = cfg.group(&mockingbird::DEMIX_GROUP);
    }

    #[cfg(feature = "basic-cmds")]
    { cfg = cfg.group(&basic::COMMANDS_GROUP); }

    #[cfg(feature = "list-feature-cmd")]
    { cfg = cfg.group(&features::FEATURES_GROUP); }

    #[cfg(feature = "prerelease")]
    { cfg = cfg.group(&features::PRERELEASE_GROUP); }

    cfg
}

#[allow(unused_mut)]
pub fn setup_state(mut cfg: ClientBuilder) -> ClientBuilder {
    #[cfg(feature = "mockingbird")]
    {
        use songbird::SerenityInit;
        cfg = cfg.register_songbird();

        #[cfg(feature = "demix")]
        {
            use mockingbird::demix::{Demix, ArlToken};
            cfg = cfg.type_map_insert::<ArlToken>(String::from(arl));
        }
    }

    cfg
}

pub struct EvHandler;
#[async_trait]
impl EventHandler for EvHandler {

    #[allow(unused_variables)]
    async fn reaction_add(&self, ctx: Context, ev: Reaction) {
        #[cfg(feature="bookmark-emoji")]
        tokio::spawn(async {
            use bookmark::bookmark_on_react_add;
            match bookmark_on_react_add(&ctx, &ev).await {
                Ok(_) => {},
                Err(e) => { ev.channel_id.say(&ctx.http, format!("Error: {}", e)).await.unwrap(); },
            };
        });
    }

    #[allow(unused_variables)]
    async fn message(&self, ctx: Context, msg: Message) {
        #[cfg(feature="enable-dj-room")]
        tokio::spawn(async {
            const DJ_CHANNEL: u64 = 960044319476179055;
            let bot_id = ctx.cache.current_user_id().0;
            if msg.channel_id.0 == DJ_CHANNEL && msg.author.id.0 != bot_id {
                match mockingbird::on_dj_channel(&ctx, &msg).await {
                    Ok(_) => {},
                    Err(e) => { msg.channel_id.say(&ctx.http, format!("Error: {}", e)).await.unwrap(); },
                }
            }
        });
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}
