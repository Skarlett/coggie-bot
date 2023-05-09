use crate::{
    get_rev, VERSION, REPO,
};

use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};

use serenity::model::channel::Message;
use serenity::prelude::*;

#[group]
#[commands(version, rev_cmd, contribute)]
pub struct Commands;

#[command]
async fn version(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, VERSION).await?;
    Ok(())
}

#[command("rev")]
async fn rev_cmd(ctx: &Context, msg: &Message) -> CommandResult {
    match get_rev() {
        "canary" => {
            msg.channel_id.say(&ctx.http, "This is a canary build.").await?;
        },
        _ => { msg.channel_id.say(&ctx.http, format!("{REPO}/commit/{}", get_rev())).await?; }
    }
    Ok(())
}

#[command]
async fn contribute(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
       .send_message(&ctx.http, |m|
           m
            .add_embed(|e|
                e.title("Coggie Bot")
                   .description("Coggie Bot is an open source \"Discord\" (discord.com) bot.")
                   .url(REPO)
                    .fields(vec![
                        ("License", "BSD2", false),
                        ("Version", VERSION, false),
                        ("Revision", get_rev(), false),
                        ("Tickets", &format!("{}/issues", REPO), false),
                   ])
            )
       ).await?;
    Ok(())
}
