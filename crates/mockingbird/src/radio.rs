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
    process::Stdio,
    time::{Duration, Instant},
    collections::VecDeque,
    sync::Arc,
    collections::HashMap,
    path::PathBuf,
};

use tokio::io::AsyncBufReadExt;

use songbird::input::cached::Compressed;
use core::sync::atomic::Ordering;

use cutils::{availbytes, bigpipe, max_pipe_size};

#[cfg(feature = "deemix")]
use crate::deemix::{DeemixMetadata, _deemix};

use crate::models::*;
#[group]
#[commands(radio, seed)]
pub struct Radio;


#[command]
#[only_in(guilds)]
/// @bot radio [on/off/(default: status)]
async fn radio(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let mut _qctx_lock = ctx.data.write().await;
    let mut _qctx = _qctx_lock
        .get_mut::<LazyQueueKey>()
        .expect("Expected LazyQueueKey in TypeMap");

    if let None = _qctx.get(&guild_id) {
        msg.channel_id
           .say(&ctx.http, "Not in a voice channel")
           .await?;
        return Ok(())
    }
    let qctx = _qctx.get_mut(&guild_id).unwrap();
    let act = args.remains()
        .unwrap_or("status");

    match act {
        "status" =>
            { msg.channel_id
                .say(
                    &ctx.http,
                    if qctx.cold_queue.read().await.use_radio
                    { "on" } else { "off" },
                ).await?; },

        "on" => {
            qctx.cold_queue.write().await.use_radio = true;
            msg.channel_id
               .say(&ctx.http, "Radio enabled")
               .await?;
        }
        "off" => {
            let mut lock = qctx.cold_queue.write().await;
            lock.radio_queue.clear();
            lock.use_radio = false;

            msg.channel_id
               .say(&ctx.http, "Radio disabled")
               .await?;
        }
        _ => {}
    }
    Ok(())
}

#[command]
#[only_in(guilds)]
/// @bot seed [on/off/(default: status)/uri]
async fn seed(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let mut _qctx_lock = ctx.data.write().await;
    let mut _qctx = _qctx_lock
        .get_mut::<LazyQueueKey>()
        .expect("Expected LazyQueueKey in TypeMap");

    if let None = _qctx.get(&guild_id) {
        msg.channel_id
           .say(&ctx.http, "Not in a voice channel")
           .await?;
        return Ok(())
    }

    let qctx = _qctx.get_mut(&guild_id).unwrap();
    let act = args.remains()
        .unwrap_or("status");

    match act {
        "status" =>
            { msg.channel_id
                .say(
                    &ctx.http,
                    qctx.cold_queue.read().await.radio_queue.clone().into_iter().collect::<Vec<_>>().join("\n")
                ).await?; },

        _ => {}
    }

    Ok(())
}

async fn recommend(isrcs: &Vec<String>, limit: u8) -> std::io::Result<VecDeque<String>> {
    let mut buffer = std::collections::HashSet::new();

    tracing::info!("running spotify-recommend -l {} {}", limit, isrcs.join(" "));
    let recommend = tokio::process::Command::new("spotify-recommend")
        .arg("-l")
        .arg(format!("{}", limit))
        .args(isrcs.iter())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let output = recommend.wait_with_output()
        .await?;

    let mut lines = output.stdout.lines();

    while let Some(x) = lines.next_line().await? {
        buffer.insert(x);
    }
    tracing::info!("spotify-stream finished [{}]", buffer.len());
    let mut ret = VecDeque::new();
    for x in buffer {
        ret.push_back(x);
    }
    Ok(ret)
}

async fn seed_from_history(has_played: &VecDeque<TrackRecord>) -> std::io::Result<VecDeque<String>> {
    let seeds =
        has_played
            .iter()
            // Don't include skipped tracks
            .filter(|x| x.stop_event != EventEnd::Skipped)
            .filter_map(|x|
                match &x.metadata {
                    MetadataType::Deemix(meta) => meta.isrc.clone(),
                    _ => None
                })
            .collect::<Vec<_>>();


    if seeds.is_empty() {
        return Ok(seeds.into());
    }

    return recommend(&seeds, 5).await;

}

async fn preload_radio_track(
    cold_queue: &mut ColdQueue
) -> Result<(), String> {
    // pop seeds in radio
    let mut tries = 5;
    // attempts/tries loop
    loop {
        let uri = match cold_queue.radio_queue.pop_front() {
            Some(x) => Some(x),
            None => {
                cold_queue.radio_queue.clear();
                cold_queue.radio_queue.extend(seed_from_history(&cold_queue.has_played).await.unwrap_or_else(|_| VecDeque::new()));
                cold_queue.radio_queue.pop_front()
            }
        };

        if let Some(uri) = uri {
            match _deemix(&uri, &[], false, crate::deemix::DeemixLoadMethod::Mem).await {
                Ok((preload_input, metadata)) => {
                        cold_queue.radio_next = Some((Compressed::new(
                            preload_input,
                            songbird::driver::Bitrate::BitsPerSecond(128_000)
                        ).unwrap(),

                        metadata.map(|x| x.into())
                    ));
                    return Ok(())
                }

                Err(why) =>  {
                    tries -= 1;
                    tracing::error!("Error preloading radio track: {}", why);
                    if 0 >= tries {
                        return Err("Exceeded max tries".to_string());
                    }
                    continue
                }
            }
        }
        return Err("Fall through".to_string());
    }
}

pub struct RadioInvoker(Arc<QueueContext>);
impl RadioInvoker {
    pub fn new(qctx: Arc<QueueContext>) -> Self {
        Self(qctx)
    }
}

#[async_trait]
impl VoiceEventHandler for RadioInvoker {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        if let Some(call) = self.0.manager.get(self.0.guild_id) {
            let mut call = call.lock().await;
            let mut cold_queue = self.0.cold_queue.write().await;
            let crossfade = self.0.crossfade.load(Ordering::Relaxed);

            // `PreloadInvoker` may have placed a track (from the user queue)
            // before this event was fired.
            // If true, we clear our trackers.

            if ! crossfade {
                if let Some(_current_track_handle) = call.queue().current() {
                    // do nothing
                }
            }


            // `PreloadInvoker` has not placed anything,
            // lets fire it's routine on our thread.
            //
            else if let Ok(Some((track, handle, metadata))) = crate::player::next_track_handle(&mut cold_queue, self.0.clone(), crossfade).await {
                crate::player::play(&mut call, track, crossfade).await;
                // do nothing.
            }


            // else
            // // If all else fails, play the preloaded track on radio
            // else if cold_queue.use_radio {
            //    // if the user queue is empty, try the preloaded radio track
            //     if let Some((radio_preload, metadata)) = cold_queue.radio_next.take() {

            //         // play_preload_radio_track(&mut call, radio_preload, metadata, self.0.clone()).await;
            //         // let _ = preload_radio_track(&mut cold_queue).await;
            //         return None;
            //     }
            // }

            // cold_queue.radio_next = None;
            // let _ = preload_radio_track(&mut cold_queue).await;
        }
        None
    }
}