use serenity::{
    async_trait, client::Cache, framework::standard::{
        macros::{command, group}, Args, CommandResult
    }, http::Http, json, model::{channel::Message, prelude::*}, prelude::*, FutureExt
};

use songbird::{
    events::{Event, EventContext}, 
    EventHandler as VoiceEventHandler, Songbird, TrackEvent
};

use std::{
    process::Stdio,
    time::{Duration, Instant},
    collections::VecDeque,
    sync::Arc,
    collections::HashMap,
    path::PathBuf,
};

use core::sync::atomic::Ordering;

use cutils::{availbytes, bigpipe, max_pipe_size};

#[cfg(feature = "deemix")]
use crate::deemix::{DeemixMetadata, _deemix};

use crate::{models::*, player::next_track_handle};

pub struct AbandonedChannel(pub Arc<QueueContext>);
#[async_trait]
impl VoiceEventHandler for AbandonedChannel {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let members = self.0.voice_chan_id.members(&self.0.cache).await.unwrap();
        if members.iter().filter(|x| !x.user.bot).count() > 0 {
            return None;
        }

        crate::player::leave_routine(
            self.0.data.clone(),
            self.0.guild_id.clone(),
            self.0.manager.clone()
        ).await.unwrap();

        Some(Event::Cancel)
    }
}

pub struct PreloadInvoker(Arc<QueueContext>);
impl PreloadInvoker {
    pub fn new(qctx: Arc<QueueContext>) -> Self {
        Self(qctx)
    }
}

#[async_trait]
impl VoiceEventHandler for PreloadInvoker {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {      
        if let Some(call) = self.0.manager.get(self.0.guild_id) {
            let mut call = call.lock().await;
            let mut cold_queue = self.0.cold_queue.write().await;
            let crossfade = self.0.crossfade.load(Ordering::Relaxed);

            if let Ok(Some((track, _handle, _metadata))) = crate::player::next_track_handle(
                &mut cold_queue,
                self.0.clone(),
                crossfade
            ).await {
                crate::player::play(&mut call, track, self.0.crossfade.load(Ordering::Relaxed)).await;
            }
            
        }
        None
    }
}

pub struct RemoveTempFile(PathBuf);
#[async_trait]
impl VoiceEventHandler for RemoveTempFile {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let _ = tokio::fs::remove_file(&self.0).await;
        None
    }
}

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


        if let None = cold_queue.crossfade_rhs.as_ref() {
            if let Ok(Some((track, handle, _metadata))) = crate::player::next_track_handle(
                &mut cold_queue,
                self.0.clone(),
                crossfading
            ).await
            {
                let metadata = handle.metadata().clone();
                let duration = metadata.duration.unwrap();

                // let _ = handle.pause();
                // play(&mut call, track, ).await;


                let _ = handle.set_volume(0.001);
                let _ = handle.play();
                cold_queue.crossfade_rhs = Some(handle);
            }
        }

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
