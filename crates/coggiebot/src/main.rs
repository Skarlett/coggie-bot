mod controllers;

use std::env;
use serenity::{
    http::Http,
    framework::standard::{
        StandardFramework,
        DispatchError,
        macros::hook
    }, model::prelude::UserId, client::ClientBuilder
};

use serenity::model::channel::Message;
use serenity::prelude::*;
use structopt::StructOpt;
use tokio::sync::oneshot::{Receiver, Sender};
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;


pub const LICENSE:  &'static str = include_str!("../LICENSE");
pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

// TODO: FIXME: use env!("ORIGIN_REPOSITORY") instead of hardcoding the repo
pub const REPO: &'static str = "https://github.com/skarlett/coggie-bot";

pub fn get_rev() -> &'static str {
    option_env!("REV").unwrap_or("canary")
}

#[derive(Debug)]
pub enum CoggieProc {
    Kill,
    RestartClient,
    UntilSignal(oneshot::Receiver<()>),
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

    #[cfg(feature="list-feature-cmd")]
    #[structopt(long = "list-features", alias="features-list")]
    /// list all features if enabled
    features: bool,

    /// Access Token
    #[structopt(long = "token", env = EnvVars::DISCORD_TOKEN)]
    token: String,
}

#[hook]
async fn dispatch_error(_ctx: &Context, _msg: &Message, error: DispatchError, _command_name: &str) {
    tracing::error!("Error: {:?}]", error);
}

async fn mkClient(token: &str, bot_id: UserId) -> ClientBuilder
{
    let framework = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(true)
                .ignore_bots(true)
                .prefix(".")
                .on_mention(Some(bot_id))
                .delimiters(vec![", ", ","])
                .owners(std::collections::HashSet::new())
        })
        .on_dispatch_error(dispatch_error);

    let framework = controllers::setup_framework(framework);

    controllers::setup_state(
        Client::builder(token, GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .framework(framework)
        .event_handler(controllers::EvHandler))
        .await
}

struct ProcCtlKey;
impl TypeMapKey for ProcCtlKey {
    type Value = Sender<CoggieProc>;
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

    #[cfg(feature="list-feature-cmd")]
    if cli.features {
        println!("{}", controllers::features::feature_list()
            .iter()
            .map(|(f, toggle)| format!("{}: {}", f, toggle))
            .collect::<Vec<_>>()
            .join("\n"));
        return Ok(());
    }

    println!("{}", LICENSE);

    tracing_subscriber::fmt::init();   
    loop {
        let http = Http::new(&cli.token);
        let bot_id = http.get_current_user().await?.id;

        // this future never returns
    
        let client = mkClient(&cli.token, bot_id).await;
        
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        
        let client = client.type_map_insert::<ProcCtlKey>(tx);
        let mut client = client.await?;

        tokio::select! {
            _ = client.start() => {},
            ev = rx => match ev {
                Ok(CoggieProc::Kill) => {
                    tracing::info!("Killing client");
                    // client.kill().await;
                    break;
                },
                Ok(CoggieProc::RestartClient) => {
                    tracing::info!("Restarting client");
                    break;
                },
                
                Ok(CoggieProc::UntilSignal(rx)) => {
                    tracing::info!("Halting client until signal");
                    tracing::info!("Received signal, restarting client");
                    let _ = rx.await; 
                }
                
                _ => todo!(),
                Err(_) => {}
            }   
        }
    }

    Ok(())
}
