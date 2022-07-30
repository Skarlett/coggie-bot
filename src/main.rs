use std::env;

use serenity::async_trait;
use serenity::model::channel::{Message, ReactionType};
use serenity::model::gateway::Ready;
use serenity::prelude::*;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn reaction_add(ctx: Context, ev: Reaction)
    {
        if let ReactionType::Unicode(x) = ev.emoji
        {
            if !x.eq(String::from("\u{1F516}")) {
                return;
            }

            if let Some(user_id) = ev.user_id {
                let msg = ev.channel_id.message(&ctx, ev.message_id);

                let dm = user_id.to_user(&ctx)
                       .expect("Couldn't find user")
                       .create_dm_channel(&ctx)
                       .send_message(|b|
                            m.content("Hello, World!")
                             .embed(|e| {
                                 e.title("Bookmark")
                                  .description("You bookmarked this message on {} from channel {} ({})")
                                  .fields(vec![
                                      ("link:", format!("", ev.message_id.link_ensured(&ctx)), true),
                                      ("content:", msg.content, false),
                                  ])
                                  .footer(|f| f.text("https://github.com/Skarlett/coggie-bot"))
                                  .timestamp(Timestamp::now())
                             })
                       )
                       .await;
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client =
        Client::builder(&token, intents).event_handler(Handler).await.expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
