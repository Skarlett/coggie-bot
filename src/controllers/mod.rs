use std::env;

use serenity::model::prelude::Message;
use serenity::{framework::StandardFramework, client::ClientBuilder};
use serenity::async_trait;
use serenity::model::{channel::Reaction, gateway::Ready};

use serenity::prelude::*;

#[cfg(feature = "basic-cmds")]
#[path = "bookmark.rs"]
mod bookmark;

#[cfg(feature = "mockingbird")]
#[path = "mockingbird.rs"]
mod mockingbird;

#[cfg(feature = "basic-cmds")]
#[path = "basic.rs"]
mod basic;

pub fn setup_framework(mut cfg: StandardFramework) -> StandardFramework {
    #[cfg(feature = "mockingbird")]
    {
        cfg = cfg.group(&mockingbird::MOCKINGBIRD_GROUP);

        #[cfg(feature = "demix")]
        cfg = cfg.group(&mockingbird::DEMIX_GROUP);

        cfg
    }

    #[cfg(feature = "basic-cmds")]
    { cfg = cfg.group(&basic::COMMANDS_GROUP); }

    cfg
}

pub fn setup_state(mut cfg: ClientBuilder, arl: String) -> ClientBuilder {
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
    async fn reaction_add(&self, ctx: Context, ev: Reaction) {
        #[cfg(feature="bookmark")]
        tokio::spawn(async {
            use bookmark::bookmark_on_react_add;
            match bookmark_on_react_add(&ctx, &ev).await {
                Ok(_) => {},
                Err(e) => { ev.channel_id.say(&ctx.http, format!("Error: {}", e)).await.unwrap(); },
            };
        });
    }

    async fn message(&self, ctx: Context, msg: Message) {
        #[cfg(feature="enable-dj-room")]
        async {
            const DJ_CHANNEL: u64 = 960044319476179055;
            let bot_id = ctx.cache.current_user_id().0;
            if msg.channel_id.0 == DJ_CHANNEL && msg.author.id.0 != bot_id {
                match mockingbird::on_dj_channel(&ctx, &msg).await {
                    Ok(_) => {},
                    Err(e) => { msg.channel_id.say(&ctx.http, format!("Error: {}", e)).await.unwrap(); },
                }
            }
        }.await;
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}
