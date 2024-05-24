use serenity::{
    async_trait,  framework::standard::{
        macros::{command, group}, Args, CommandResult
    },
    model::channel::Message,
    prelude::*, 
};

use songbird::{
        events::{Event, EventContext}, input::Metadata, tracks::TrackHandle, EventHandler as VoiceEventHandler
};

use std::sync::Arc;
use std::time::Duration;

use core::sync::atomic::Ordering;

use crate::models::*;

trait CrossoverHandler {
    fn handler(&self, current: &TrackHandle, upcoming: &TrackHandle) -> Result<(), HandlerError> {
        Ok(())
    }

    fn start_at(&self, metadata: &Metadata) -> Duration {
        metadata.duration
            .map(|x| x - Duration::from_secs(10))
            .unwrap_or(Duration::from_secs(0))
    }
}


#[group]
#[commands(crossfade)]
struct Crossfade;

pub struct CrossFadeInvoker(pub Arc<QueueContext>);
#[async_trait]
impl VoiceEventHandler for CrossFadeInvoker {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        if let None = self.0.manager.get(self.0.guild_id) {
            return Some(Event::Cancel)
        }

        let mut cold_queue = self.0.cold_queue.write().await;
        let peak = 10000;
        let root : i32 = (peak as f32).sqrt() as i32;
        // let step = 1;
        let mut cold_queue = tokio::task::block_in_place(move || {
            for x in 0 ..= root {
                let fade_out = peak - x.pow(2);
                let fade_out_normal = fade_out as f32 / 10000.0;
                let fade_in_normal = (peak - fade_out) as f32 / 10000.0;

                tracing::info!("crossfade: fade_out: {}, fade_in: {}", fade_out_normal, fade_in_normal);

                match (cold_queue.crossfade_lhs.as_ref(), cold_queue.crossfade_rhs.as_ref()) {
                    (Some(lhs), Some(rhs)) => {
                        let _ = rhs.play();
                        let _ = lhs.set_volume(fade_out_normal);
                        let _ = rhs.set_volume(fade_in_normal);
                    }

                    (Some(_lhs), None) => {
                        // let _ = lhs.set_volume(fade_out);
                    },

                    (None, Some(rhs)) => {
                        let _ = rhs.set_volume(fade_in_normal);
                    },

                    (None, None) => {
                        break
                    }
                    // return Some(Event::Cancel)
                }

                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            cold_queue
        });

        if let Some(rhs) = cold_queue.crossfade_rhs.take() {
            // x.stop();
            if let Some(lhs) = cold_queue.crossfade_lhs.take() {
                let _ = lhs.stop();
            }
            cold_queue.crossfade_lhs.replace(rhs);
        }
        else { cold_queue.crossfade_lhs = None; }
        return None 
    }
}


#[command]
#[only_in(guilds)]
/// @bot radio [on/off/(default: status)]
async fn crossfade(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
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
                    if qctx.crossfade.load(Ordering::Relaxed)
                    { "on" } else { "off" },
                ).await?; },

        "on" => {
            qctx.crossfade.swap(true, Ordering::Relaxed);
            msg.channel_id
               .say(&ctx.http, "crossfade enabled")
               .await?;
        }
        "off" => {
            let mut lock = qctx.cold_queue.write().await;
            lock.radio_queue.clear();
            lock.use_radio = false;

            msg.channel_id
               .say(&ctx.http, "crossfade disabled")
               .await?;
        }
        _ => {}
    }
    Ok(())
}
