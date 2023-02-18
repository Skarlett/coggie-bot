use std::{env, thread};

use std::time::{Duration, SystemTime};
use serde_json;
use crossbeam_channel::Receiver;
use serenity::async_trait;
use serenity::builder::CreateMessage;
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, StandardFramework,
};
use serenity::http::Http;
use serenity::model::{
    channel::{Message, ReactionType},
    gateway::Ready,
    prelude::Reaction,
    Timestamp,
};

use serenity::prelude::*;
use structopt::StructOpt;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::time::{sleep, Instant};

const LICENSE: &'static str = include_str!("../LICENSE");
const REPO: &'static str = "https://github.com/skarlett/coggie-bot";
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn get_rev() -> &'static str {
    option_env!("REV").unwrap_or("canary")
}

#[group]
#[commands(version, rev_cmd, tester_cmd, contribute)]
struct Commands;

#[command]
async fn version(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, VERSION).await?;
    Ok(())
}

#[command("rev")]
async fn rev_cmd(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .say(&ctx.http, format!("{REPO}/commit/{}", get_rev()))
        .await?;
    Ok(())
}


#[command("tester")]
async fn tester_cmd(ctx: &Context, msg: &Message) -> CommandResult {

    let mut file = File::open("tips.json").await.unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).await.ok();
    let json: Option<String> = serde_json::from_str(&data).ok();
    
    
    msg.channel_id
        .say(&ctx.http, format!("{}", json.unwrap()))
        .await?;
    Ok(())
}


#[command]
async fn contribute(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.add_embed(|e| {
                e.title("Coggie Bot")
                    .description("Coggie Bot is an open source \"Discord\" (discord.com) bot.")
                    .url(REPO)
                    .fields(vec![
                        ("License", "BSD2", false),
                        ("Version", VERSION, false),
                        ("Revision", get_rev(), false),
                        ("Contribute", &format!("{}/contribute.md", REPO), false),
                    ])
            })
        })
        .await?;
    Ok(())
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn reaction_add(&self, ctx: Context, ev: Reaction) {
        if let ReactionType::Unicode(x) = ev.emoji {
            /* :bookmark: */
            if x != "\u{1F516}" {
                return;
            }

            if let Some(user_id) = ev.user_id {
                // grab message
                let msg = ev.channel_id.message(&ctx, ev.message_id).await.unwrap();

                // build response
                let link = match ev.guild_id {
                    Some(gid) => format!(
                        "https://discord.com/channels/{}/{}/{}",
                        gid, ev.channel_id, ev.message_id
                    ),
                    None => String::from("N/A"),
                };

                let attachments = match msg.attachments.is_empty() {
                    true => String::from("N/A"),
                    false => msg
                        .attachments
                        .iter()
                        .map(|c| format!("{}\n", c.url))
                        .collect::<String>(),
                };

                let content = match msg.content.as_str() {
                    "" => String::from("N/A"),
                    _ => msg.content.to_string(),
                };

                user_id
                    .to_user(&ctx)
                    .await
                    .expect("Couldn't find user")
                    .create_dm_channel(&ctx)
                    .await
                    .unwrap()
                    .send_message(&ctx, |m: &mut CreateMessage| {
                        m.add_embed(|e| {
                            e.title("Bookmark")
                                .fields(vec![
                                    ("author:", msg.author.tag(), false),
                                    ("attachments:", attachments, false),
                                    ("content:", content, false),
                                    ("link:", link, true),
                                ])
                                .footer(|f| f.text(REPO))
                                .timestamp(Timestamp::now())
                        });

                        let m = msg
                            .attachments
                            .iter()
                            .map(|c| c.url.clone())
                            .chain(
                                msg.content
                                    .split_whitespace()
                                    .filter(|x| x.starts_with("http"))
                                    .map(|x| x.to_string()),
                            )
                            .filter_map(|a| {
                                if let Some((_prefix, suffix)) = &a.rsplit_once('.') {
                                    Some((a.clone(), suffix.to_string()))
                                } else {
                                    None
                                }
                            })
                            .fold(m, |msg, (atch, ext)| match ext.as_str() {
                                "png" | "jpg" | "jpeg" | "gif" => {
                                    msg.add_embed(|e| e.image(&atch));
                                    msg
                                }
                                _ => msg,
                            });
                        m
                    })
                    .await
                    .unwrap();
            }
        }
        //println!("received: {}", self.receiver.recv().ok().unwrap())
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
    token: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let (tx, rx) = crossbeam_channel::bounded(32);
    let start_time = Instant::now();
    
    
    // time tracking thread for interval based functionality
    let delta_thr = tokio::spawn(async move {
        loop {
            let current_time = start_time.elapsed();
            println!("{:?}", current_time.as_millis());
            sleep(Duration::from_millis(1000)).await;
            tx.send(current_time.as_secs() % 5);
            tx.send(current_time.as_secs());
            
            
        }
    });
    
    let framework = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(true)
                .ignore_bots(true)
                .prefix(".")
                .on_mention(Some(bot_id))
                .delimiters(vec![", ", ","])
                .owners(std::collections::HashSet::new())
        })
        .group(&COMMANDS_GROUP);

    let mut client = Client::builder(&cli.token, GatewayIntents::non_privileged())
        .framework(framework)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    client.start().await?;
    unreachable!();
}
