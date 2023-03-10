mod controllers;
mod pkglib;
mod config;
use std::env;

use serenity::framework::standard::StandardFramework;
use serenity::http::Http;

use serenity::prelude::*;
use structopt::StructOpt;

pub const LICENSE:  &'static str = include_str!("../LICENSE");
pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

// TODO: FIXME: use env!("ORIGIN_REPOSITORY") instead of hardcoding the repo
pub const REPO: &'static str = "https://github.com/skarlett/coggie-bot";

pub fn get_rev() -> &'static str {
    option_env!("REV").unwrap_or("canary")
}

/// Environment variables used at runtime to
/// determine attributes of the program.
#[allow(non_snake_case)]
pub mod EnvVars {
    pub const DISCORD_TOKEN: &'static str = "DISCORD_TOKEN";
    pub const CONFIG_FILE: &'static str = "CONFIG_FILE";
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

    // #[cfg(feature="list-feature-cmd")]
    // #[structopt(long = "list-features", alias="features-list")]
    // /// list all features if enabled
    // features: bool,

    /// Access Token
    #[structopt(long = "token", env = EnvVars::DISCORD_TOKEN)]
    token: String,

    #[structopt(long = "config", env = EnvVars::CONFIG_FILE)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>
{
    let conf = config::Configuration::default();

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
    #[cfg(feature="list-feature-cmd")]
    if cli.features {
        println!("{:?}", controllers::features::feature_list());
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

    let mut client = controllers::setup_state(
        Client::builder(&cli.token, GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .framework(framework)
        .event_handler(controllers::EvHandler))
        .await?;

    client.start().await?;
    unreachable!();
}
