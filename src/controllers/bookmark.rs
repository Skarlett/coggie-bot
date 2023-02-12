use serenity::builder::CreateMessage;
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, StandardFramework,
};
use crate::REPO;
use serenity::model::{
    channel::{Message, ReactionType},
    gateway::Ready,
    prelude::Reaction,
    Timestamp,
};

use serenity::prelude::*;
use structopt::StructOpt;


pub async fn bookmark_on_react_add(ctx: &Context, ev: &Reaction) -> CommandResult {
    if let ReactionType::Unicode(x) = &ev.emoji {

        /* :bookmark: */
        if x != "\u{1F516}" {
            return Ok(());
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
                        .map(|c| c.url.clone())
                        .chain(
                            msg.content
                                .split_whitespace()
                                .filter(|x| x.starts_with("http"))
                                .map(|x| x.to_string())
                        )

                        .filter_map(|a| if let Some((_prefix, suffix)) = &a.rsplit_once('.') {
                            Some((a.clone(), suffix.to_string()))
                        } else { None })

                        .fold(m, |msg, (atch, ext)|
                            match ext.as_str() {
                                "png" | "jpg" | "jpeg" | "gif" => { msg.add_embed(|e| e.image(&atch)); msg},
                                _ => msg
                            }
                        );
                    m
                })
                .await
                .unwrap();
        }
    }
    return Ok(());
}
