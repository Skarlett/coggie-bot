use std::env;

use serenity::async_trait;
use serenity::model::channel::{ReactionType};
use serenity::model::gateway::Ready;
use serenity::model::prelude::Reaction;
use serenity::model::Timestamp;
use serenity::builder::CreateMessage;
use serenity::prelude::*;
struct Handler;

#[async_trait]
impl EventHandler for Handler
{
    async fn reaction_add(&self, ctx: Context, ev: Reaction)
    {
        if let ReactionType::Unicode(x) = ev.emoji
        {
            if x != "\u{1F516}" {
                return;
            }

            if let Some(user_id) = ev.user_id {
                let link: String = {
                    if let Some(gid) = ev.guild_id {
                        format!("https://discord.com/channels/{}/{}/{}",
                                gid, ev.channel_id, ev.message_id)
                    }
                    else {
                        String::from("N/A")
                    }
                };

                let msg = ev.channel_id
                    .message(&ctx, ev.message_id).await.unwrap();

                let attachments = {
                    if msg.attachments.len() > 0 {
                        msg.attachments
                           .iter()
                           .map(|c| format!("{}\n", c.url))
                           .collect::<String>()
                    }
                    else {
                        String::from("N/A")
                    }
                };

                let content = {
                    if msg.content.len() > 0 {
                        msg.content.to_string()
                    }
                    else {
                        String::from("N/A")
                    }
                };

                user_id.to_user(&ctx)
                    .await
                    .expect("Couldn't find user")
                    .create_dm_channel(&ctx)
                    .await
                    .unwrap()
                    .send_message(&ctx, |m: &mut CreateMessage| {
                            m.embed(|e| {
                                e.title("Bookmark")
                                // .description(
                                //     format!("You bookmarked this message on {} from channel {} ({})"))
                                .fields(vec![
                                    ("author:", msg.author.tag(), false),
                                    ("attachments:", attachments, false),
                                    ("content:", content , false),
                                    ("link:", link, true),
                                ])
                                .footer(|f| f.text("https://github.com/Skarlett/coggie-bot"))
                                .timestamp(Timestamp::now())
                            })
                        }
                    )
                    .await
                    .unwrap();
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    dotenv::dotenv()?;

    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MESSAGE_REACTIONS;

    let mut client =
        Client::builder(&token, intents)
            .event_handler(Handler)
            .await
            .expect("Err creating client");

    client.start().await?;
    unreachable!();
}
