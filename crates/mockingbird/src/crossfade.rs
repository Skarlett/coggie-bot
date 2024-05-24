use serenity::{
    async_trait,  framework::standard::{
        macros::{command, group}, Args, CommandResult
    },
    model::channel::Message,
    prelude::*, 
};

use songbird::{
        events::{Event, EventContext}, 
        EventHandler as VoiceEventHandler
};

use std::sync::Arc;


use core::sync::atomic::Ordering;

use crate::models::*;

#[group]
#[commands(crossfade)]
struct Crossfade;

pub struct CrossFadeInvoker(pub Arc<QueueContext>);
#[async_trait]
impl VoiceEventHandler for CrossFadeInvoker {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        // const total_duration: std::time::Duration = std::time::Duration::from_secs(10);
        let crossfading = self.0.crossfade.load(Ordering::Relaxed);

        let mut cold_queue = self.0.cold_queue.write().await;
        let peak = 10000;
        let root : i32 = (peak as f32).sqrt() as i32;
        let step = 1;

        if let None = self.0.manager.get(self.0.guild_id) {
            return Some(Event::Cancel)
        }

        let manager = self.0.manager.get(self.0.guild_id).unwrap();
        let mut call = manager.lock().await;


        // if let None = cold_queue.crossfade_rhs.as_ref() {
        //     if let Ok(Some((track, handle, _metadata))) = crate::player::next_track_handle(
        //         &mut cold_queue,
        //         self.0.clone(),
        //         crossfading
        //     ).await
        //     {
        //         let metadata = handle.metadata().clone();
        //         let duration = metadata.duration.unwrap();

        //         // let _ = handle.pause();
        //         // play(&mut call, track, ).await;


        //         let _ = handle.set_volume(0.001);
        //         let _ = handle.play();
        //         cold_queue.crossfade_rhs = Some(handle);
        //     }
        // }

        let x = {
            let mut lock = self.0.crossfade_step.lock();
            let x = *lock;
            *lock += step;
            if x > root as i32 {
                cold_queue.crossfade_lhs = cold_queue.crossfade_rhs.take();
                return Some(Event::Cancel);
            }
            x
        };

        let fade_out = (peak - x.pow(2)) as f32 / 100.0;
        let fade_in = (peak as f32 - fade_out) / 100.0;

        match (cold_queue.crossfade_lhs.as_ref(), cold_queue.crossfade_rhs.as_ref()) {
            (Some(lhs), Some(rhs)) => {
                let _ = lhs.set_volume(fade_out);
                let _ = rhs.set_volume(fade_in);
            }

            (Some(lhs), None) => {
                let _ = lhs.set_volume(fade_out);
            },

            (None, Some(rhs)) => {
                let _ = rhs.set_volume(fade_in);
            },

            (None, None) => return Some(Event::Cancel)
        }
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
