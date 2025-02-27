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
#[commands(version, rev_cmd, contribute, reboot, invite)]
pub struct Commands;

#[command]
async fn version(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, VERSION).await?;
    Ok(())
}

#[command]
async fn reboot(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, "will kill all hu-").await?;
    std::process::exit(3);
    panic!("reboot request");
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
#[command("invite")]
async fn invite(ctx: &Context, msg: &Message) -> CommandResult {
    let mut builder = serenity::builder::CreateBotAuthParameters::default();

    msg.channel_id.say(&ctx.http,
        <serenity::builder::CreateBotAuthParameters as Clone>::clone(&builder
          .auto_client_id(&ctx.http).await?
          .scopes(&[serenity::model::prelude::Scope::Bot]))
          .build()
    ).await?;
    Ok(())
}