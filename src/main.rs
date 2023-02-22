mod controllers;
// mod ncomplex;
use std::env;

use serenity::framework::standard::StandardFramework;
use serenity::http::Http;

use serenity::prelude::*;
use structopt::StructOpt;

pub const LICENSE:  &'static str = include_str!("../LICENSE");
pub const REPO: &'static str = "https://github.com/skarlett/coggie-bot";
pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub fn get_rev() -> &'static str {
    option_env!("REV")
        .unwrap_or("canary")
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

    let arl_token = std::env::var("ARL_TOKEN")
        .expect("ARL_TOKEN must be set");

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

    let mut client = controllers::setup_state(
        Client::builder(&cli.token, GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .framework(framework)
        .event_handler(controllers::EvHandler), arl_token )
        .await?;

    client.start().await?;
    unreachable!();
}
