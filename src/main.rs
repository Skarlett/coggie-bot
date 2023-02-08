use std::env;

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

const LICENSE:  &'static str = include_str!("../LICENSE");
const REPO: &'static str = "https://github.com/skarlett/coggie-bot";
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn get_rev() -> &'static str {
    option_env!("REV")
        .unwrap_or("canary")
}

#[group]
#[commands(version, rev_cmd, contribute)]
struct Commands;

#[command]
async fn version(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, VERSION).await?;
    Ok(())
}

#[command("rev")]
async fn rev_cmd(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, format!("{REPO}/commit/{}", get_rev())).await?;
    Ok(())
}

#[command("contribute")]
async fn contribute(ctx: &Context, msg: &Message) -> CommandResult {
    msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.reference_message(msg)
             .allowed_mentions(|f| f.empty_parse())
             .embed(|e| {
                 e.title("Coggie Bot");
                 e.description("Coggie Bot is an open source bot written in Rust. It is licensed under the BSD2 license.");
                 e.url(REPO);
                 e.field("License", LICENSE, false);
                 e.field("Contribute", format!("{}#contribute", REPO), false);
                 e
             })
            .content(include!("../tickets.md"))
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

                        let m = msg.attachments
                           .iter()
                           .map(|a| (a, a.filename.rsplit_once('.').unwrap().1) )
                           .fold(m, |msg, (atch, ext)|
                               match ext {
                                   "png" | "jpg" | "jpeg" | "gif" => { msg.add_embed(|e| e.image(&atch.url)); msg},
                                   _ => msg
                               }
                           );
                        m
                    })
                    .await
                    .unwrap();
            }
        }
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

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MESSAGE_REACTIONS;

    let http = Http::new(&cli.token);
    let bot_id = http.get_current_user().await?.id;

    let framework = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(true)
                .on_mention(Some(bot_id))
                .delimiters(vec![", ", ","])
                .owners(std::collections::HashSet::new())
        })
        .group(&COMMANDS_GROUP);

    let mut client = Client::builder(&cli.token, intents)
        .framework(framework)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    client.start().await?;
    unreachable!();
}
