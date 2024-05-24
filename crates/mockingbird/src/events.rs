use serenity::async_trait;

use songbird::{
    events::{Event, EventContext}, 
    EventHandler as VoiceEventHandler, 
};

use std::{
    sync::Arc,
    path::PathBuf,
};

use core::sync::atomic::Ordering;
use crate::models::*;

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
            
            if let Ok(Some((track, handle, _metadata))) = crate::player::next_track_handle(
                &mut cold_queue,
                self.0.clone(),
                crossfade
            ).await
            {
                crate::player::play(&mut call, track, &handle, &mut cold_queue, crossfade).await;
            }
        }
        None
    }
}

pub struct RemoveTempFile(pub PathBuf);
#[async_trait]
impl VoiceEventHandler for RemoveTempFile {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let _ = tokio::fs::remove_file(&self.0).await;
        None
    }
}
