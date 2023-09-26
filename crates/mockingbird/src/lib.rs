use std::time::Duration;
use std::collections::HashMap;
use std::sync::Arc;

use serenity::model::id::GuildId;
use serenity::prelude::*;
// #[cfg(feature = "controller")]
pub mod controller;

pub mod routines;
pub mod fan;
pub mod ctrlerror;

pub mod player;

#[cfg(feature = "deemix")]
mod deemix;

#[cfg(feature = "check")]
pub mod check;

pub const TS_PRELOAD_OFFSET: Duration = Duration::from_secs(20);
pub const TS_ABANDONED_HB: Duration = Duration::from_secs(720);
pub const HASPLAYED_MAX_LEN: usize = 10;

type LazyQueue = HashMap<GuildId, Arc<player::QueueContext>>;
pub struct LazyQueueKey;
impl TypeMapKey for LazyQueueKey {
    type Value = LazyQueue;
}


#[cfg(test)]
mod testsuite;

use serenity::client::ClientBuilder;

pub async fn init(mut cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("Mockingbird initializing...");
    use songbird::SerenityInit;


    #[cfg(feature = "controller")] {
        cfg = cfg.type_map_insert::<LazyQueueKey>(HashMap::new());
    }

    cfg.register_songbird()
}
