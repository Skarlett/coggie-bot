use serenity::{framework::StandardFramework, client::ClientBuilder};
use std::env;

use serenity::async_trait;
use serenity::framework::standard::{CommandGroup};
use serenity::http::Http;
use serenity::model::{channel::Reaction, gateway::Ready};

use serenity::prelude::*;
use structopt::StructOpt;

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
    { cfg = cfg.group(&mockingbird::MOCKINGBIRD_GROUP); }

    #[cfg(feature = "basic-cmds")]
    { cfg = cfg.group(&basic::COMMANDS_GROUP); }

    cfg
}

pub fn setup_state(mut cfg: ClientBuilder) -> ClientBuilder {

    #[cfg(feature = "mockingbird")]
    {
        use songbird::SerenityInit;
        cfg = cfg.register_songbird();
    }
        // .type_map_insert::<CommandCounter>(HashMap::default())
        // .await
        // .expect("Err creating client");

    //     let mut data = client.data.write().await;
    //     data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));

    cfg
}


struct EvHandler;
#[async_trait]
impl EventHandler for EvHandler {
    async fn reaction_add(&self, ctx: Context, ev: Reaction) {
        #[cfg(feature="bookmark")]
        async {
            use bookmark::bookmark_on_react_add;
            match bookmark_on_react_add(&ctx, &ev).await {
                Ok(_) => {},
                Err(e) => { ev.channel_id.say(&ctx.http, format!("Error: {}", e)).await.unwrap(); },
            };
        };
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}
