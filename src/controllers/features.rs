use crate::{REPO, pkglib::{CoggiebotError}};
use serenity::model::prelude::Message;
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};

use std::{path::PathBuf, io::BufRead};
use serenity::prelude::*;

pub const FEATURES: &'static str = env!("COGGIEBOT_FEATURES");

pub fn feature_list() -> Vec<(String, bool)>{
    FEATURES.split(",")
        .filter_map(|s| s.split_once("="))
        .map(|(a, b)| (a.to_string(), b.parse::<i32>().expect("Invalid value for feature") >= 1))
        .collect()
}

#[group]
#[commands(features)]
pub struct Features;

#[command("features")]
async fn features(ctx: &Context, msg: &Message) -> CommandResult {
    let features = feature_list();

    msg.channel_id
       .send_message(&ctx.http, |m|
           m.add_embed(|e|
                e.title("Coggie Bot")
                 .description("Coggie Bot is an open source \"Discord\" (discord.com) bot.")
                 .url(REPO)
                 .fields(features
                    .iter().map(|r| match r {
                        (name, true) => (name, "enabled", true),
                        (name, false) => (name, "disabled", true),
                        _ => unreachable!(),
                    })
                 )
            )
       ).await?;
    Ok(())
}
