mod controllers;
use std::env;

use serenity::async_trait;
use serenity::framework::standard::{StandardFramework, CommandGroup};
use serenity::http::Http;
use serenity::model::{channel::Reaction, gateway::Ready};

use serenity::prelude::*;
use structopt::StructOpt;

#[cfg(feature = "basic-cmds")]
#[path = "controllers/bookmark.rs"]
mod bookmark;

pub const LICENSE:  &'static str = include_str!("../LICENSE");
pub const REPO: &'static str = "https://github.com/skarlett/coggie-bot";
pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub fn get_rev() -> &'static str {
    option_env!("REV")
        .unwrap_or("canary")
}

struct Handler;
#[async_trait]
impl EventHandler for Handler {
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

#[derive(Debug, StructOpt)]
#[structopt(name = "coggiebot", about = "An example of StructOpt usage.")]
struct CLI {
    /// Show Package Version
    #[structopt(short, long)]
    version: bool,

    /// Show commit hash built from
    #[structopt(long = "built-from")]
    built_from: bool,

    /// Show commit hash built from
    #[structopt(long = "license")]
    license: bool,

    /// Access Token
    #[structopt(long = "token", env = "DISCORD_TOKEN")]
    token: String
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>
{
    let cli = CLI::from_args();
    if cli.version {
        println!("{VERSION}");
        return Ok(());
    } else if cli.license {
        println!("{LICENSE}");
        return Ok(());
    } else if cli.built_from {
        println!("{}", get_rev());
        return Ok(());
    }

    println!("{}", LICENSE);

    let http = Http::new(&cli.token);
    let bot_id = http.get_current_user().await?.id;

    let framework = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(true)
                .ignore_bots(true)
                .prefix(".")
                .on_mention(Some(bot_id))
                .delimiters(vec![", ", ","])
                .owners(std::collections::HashSet::new())
        });

    let framework = controllers::setup_framework(framework);

    let mut client = controllers::setup_state(Client::builder(&cli.token, GatewayIntents::non_privileged())
        .framework(framework)
        .event_handler(Handler))
        .await?;

    client.start().await?;
    unreachable!();
}
