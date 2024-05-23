use crate::models::*;

// this is the rat nest
// be prepared
// to see how lazy i can be.
use serenity::{
    async_trait, client::Cache, framework::standard::{
        macros::{command, group}, Args, CommandResult
    }, http::Http, json, model::{channel::Message, prelude::*}, prelude::*, FutureExt
};

use songbird::{
    create_player, error::{JoinError, JoinResult}, events::{Event, EventContext, EventData}, input::{
        error::Error as SongbirdError, Input, Metadata
    }, tracks::{PlayMode, Track, TrackHandle}, Call, EventHandler as VoiceEventHandler, Songbird, TrackEvent
};

use std::{
    time::{Duration, Instant},
    collections::VecDeque,
    sync::Arc,
    collections::HashMap,
    path::PathBuf,
};

use std::sync::atomic::AtomicBool;
use tokio::{
    io::AsyncBufReadExt,
    process::Command,
    sync::oneshot::Sender
};
use parking_lot::{Mutex, MutexGuard};

use tokio::io::AsyncWriteExt;
use serenity::futures::StreamExt;
use songbird::input::cached::Compressed;
use core::sync::atomic::Ordering;

use cutils::{availbytes, bigpipe, max_pipe_size};

#[cfg(feature = "deemix")]
use crate::deemix::{DeemixMetadata, _deemix};

use crate::models::*;




#[group]
#[commands(setarl, getarl)]
struct Dangerous;

#[command]
#[only_in(guilds)]
async fn setarl(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    tracing::info!("[{}::{}] set a new arl", msg.author.id, msg.author.name);

    let arl = args.single::<String>()?;

    if !(arl.trim().len() == 192 && arl.chars().all(|c| c.is_ascii_hexdigit())) {
        msg.channel_id.say(&ctx.http, "Invalid ARL").await?;
        return Ok(())
    }

    std::env::set_var("DEEMIX_ARL", arl);
    msg.channel_id.say(&ctx.http, "**ARL has been set**").await?;

    return Ok(())
}

#[command]
#[only_in(guilds)]
async fn getarl(ctx: &Context, msg: &Message) -> CommandResult {
    tracing::info!("[{}::{}] requested arl", msg.author.id, msg.author.name);

    let arl = std::env::var("DEEMIX_ARL");
    tracing::info!("getarl: {:?}", arl);
    match arl {
        Err(e) => { msg.channel_id.say(&ctx.http, format!("Error: {}", e)).await?; }
        Ok(arl) if arl.is_empty() => { msg.channel_id.say(&ctx.http, "ARL not set").await?; }
        Ok(arl) => {
            #[cfg(feature = "check")]
            {
                msg.channel_id.say(&ctx.http, format!("getting arl data...")).await?;
                use serenity::framework::standard::{Args, Delimiter};
                let mut args = Args::new(arl.as_str(), &[Delimiter::Single(' ')]);
                crate::check::arl_check(ctx, msg, args).await?;
            }
            #[cfg(not(feature = "check"))]
            msg.channel_id.say(&ctx.http, format!("ARL: {}", &arl)).await?;
        }
    }
    return Ok(())
}
