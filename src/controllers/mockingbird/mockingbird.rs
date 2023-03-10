//! Example demonstrating how to make use of individual track audio events,
//! and how to use the `TrackQueue` system.
//!
//! Requires the "cache", "standard_framework", and "voice" features be enabled in your
//! Cargo.toml, like so:
//!
//! ```toml
//! [dependencies.serenity]
//! git = "https://github.com/serenity-rs/serenity.git"
//! features = ["cache", "framework", "standard_framework", "voice"]
//! ```

use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::Duration,
};

use std::{
    io::{BufRead, BufReader, Read},
    process::{Command, Stdio},
    default::Default
};
use tokio::{process::Command as TokioCommand, task};

use std::ffi::OsStr;
use serenity::{
    async_trait,
    client::Context,
    framework::{
        standard::{
            macros::{command, group},
            Args,
            CommandResult,
        },
    },
    http::Http,
    model::{channel::Message, prelude::ChannelId},
    prelude::Mentionable,
    Result as SerenityResult,
};

use songbird::{
    input::{
        self,
        restartable::{Restartable, Restart},
        Input,
        Container,
        Codec,
        Metadata,
        error::Error as InputError,
        children_to_reader,
    },
    Event,
    EventContext,
    EventHandler as VoiceEventHandler,
    TrackEvent,
};

#[derive(Default, Debug, Deserialize, Serialize)]
pub struct MockingbirdConfig {
    #[cfg(feature="demix")]
    demix: DemixConfig,
}

#[group]
#[commands(
    deafen, join, leave, queue, skip, stop, undeafen, unmute
)]
struct MockingBird;
pub async fn on_dj_channel(ctx: &Context, msg: &Message) -> CommandResult {
    let url = match &msg.content
    {
        url if url.starts_with("http") => url.to_string(),
        _ => {
            msg.channel_id
               .say(&ctx.http, "Must provide a URL to a video or audio")
               .await?;
            return Ok(());
        },
    };

    let guild_id = match msg.guild(&ctx.cache)
    {
        Some(guild) => guild.id,
        None => {
            msg.channel_id
               .say(&ctx.http, "Must be used in a guild")
               .await?;
            return Ok(());
        },
    };


    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        // if url.contains("deezer.page")
        // {
        //     let arl = match ctx.data.read().await.get::<ArlToken>() {
        //         Some(arl) => arl.clone(),
        //         None => {
        //             check_msg(
        //                 msg.channel_id
        //                     .say(&ctx.http, "No ARL token found")
        //                     .await,
        //             );

        //             return Ok(());
        //         }
        //     };

        //     let restarter = match deezer(&url, &arl, &[] ){
        //         Ok(src) => src,
        //         Err(e) => {
        //             check_msg(
        //                 msg.channel_id
        //                     .say(&ctx.http, format!("Error: {}", e))
        //                     .await,
        //             );
        //             return Ok(());
        //         }
        //     };

        //     handler.enqueue_source(restarter.into());
        // }
        // else
        {
            // Here, we use lazy restartable sources to make sure that we don't pay
            // for decoding, playback on tracks which aren't actually live yet.
            let source = match Restartable::ytdl(url, true).await {
                Ok(source) => source,
                Err(why) => {
                    println!("Err starting source: {:?}", why);
                    msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await;
                    return Ok(());
                },
            };
            handler.enqueue_source(source.into());
        }

        msg.channel_id
           .say(
               &ctx.http,
               format!("Added song to queue: position {}", handler.queue().len()),
           )
           .await;
    }
    else {
        msg.channel_id
           .say(&ctx.http, "Not in a voice channel to play in")
           .await;
    }

    Ok(())
}


// #[command("arl")]
// async fn get_arl(ctx: &Context, msg: &Message) -> CommandResult {
//     let arl = ctx.data.read().await.get::<ArlToken>().expect("Expected CommandCounter in TypeMap.").clone();
//     msg.channel_id.say(&ctx.http, arl).await?;
//     Ok(())
// }


#[command]
async fn deafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            msg.reply(ctx, "Not in a voice channel").await;

            return Ok(());
        },
    };

    let mut handler = handler_lock.lock().await;

    if handler.is_deaf() {
        msg.channel_id.say(&ctx.http, "Already deafened").await;
    } else {
        if let Err(e) = handler.deafen(true).await {
            (
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        msg.channel_id.say(&ctx.http, "Deafened").await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            msg.reply(ctx, "Not in a voice channel").await;
            return Ok(());
        },
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let (_handle_lock, success) = manager.join(guild_id, connect_to).await;

    let reply = match success {
        Ok(_) => format!("Joined {}", connect_to.mention()),
        Err(e) =>format!("Failed to join voice channel: {:?}", e),
    };

    msg.channel_id
       .say(&ctx.http, reply)
       .await;

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            msg.channel_id
               .say(&ctx.http, format!("Failed: {:?}", e))
               .await;
        }

        (msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        (msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn queue(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            msg.channel_id
               .say(&ctx.http, "Must provide a URL to a video or audio")
               .await?;

            return Ok(());
        },
    };

    if !url.starts_with("http") {
        msg.channel_id
           .say(&ctx.http, "Must provide a valid URL")
           .await;

        return Ok(());
    }

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    match manager.get(guild_id) {
        None => {
            msg.channel_id
               .say(&ctx.http, "Not in a voice channel to play in")
               .await?;
        }

        Some(handler_lock) =>
        {
            // Here, we use lazy restartable sources to make sure that we don't pay
            // for decoding, playback on tracks which aren't actually live yet.
            let mut handler = handler_lock.lock().await;
            let source = match Restartable::ytdl(url, true).await {
                Ok(source) => source,
                Err(why) => {
                    println!("Err starting source: {:?}", why);
                    msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await;
                    return Ok(());
                },
            };

            handler.enqueue_source(source.into());
            msg.channel_id
               .say(
                   &ctx.http,
                   format!("Added song to queue: position {}", handler.queue().len()),
               )
               .await?;
        }
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.skip();

        msg.channel_id
           .say(&ctx.http,
               format!("Song skipped: {} in queue.",
                       queue.len()),
           )
           .await;
    }
    else {
        msg.channel_id
           .say(&ctx.http, "Not in a voice channel to play in")
           .await?;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.stop();
        msg.channel_id.say(&ctx.http, "Queue cleared.").await;
    } else {
        msg.channel_id
           .say(&ctx.http, "Not in a voice channel to play in")
           .await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn undeafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.deafen(false).await {
            msg.channel_id
               .say(&ctx.http, format!("Failed: {:?}", e))
               .await?;
        }

        msg.channel_id.say(&ctx.http, "Undeafened").await;
    } else {
        msg.channel_id
           .say(&ctx.http, "Not in a voice channel to undeafen in")
           .await;
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn unmute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            msg.channel_id
               .say(&ctx.http, format!("Failed: {:?}", e))
               .await;
        }

        msg.channel_id.say(&ctx.http, "Unmuted").await?;
    } else {
        msg.channel_id
           .say(&ctx.http, "Not in a voice channel to unmute in")
           .await?;
    }

    Ok(())
}
