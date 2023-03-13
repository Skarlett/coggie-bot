use crate::{REPO, pkglib::{CoggiebotError}};
use serenity::model::prelude::Message;
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};

use std::{path::PathBuf, io::BufRead};
use serenity::prelude::*;

static RESP: &'static str = "https://tenor.com/view/ambulance-paramedic-simpsons-cliff-fall-gif-11291625";



#[group]
pub struct Help;

#[command]
async fn help(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, RESP).await?;
    Ok(())
}
