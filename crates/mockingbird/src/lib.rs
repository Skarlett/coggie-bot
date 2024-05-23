#[cfg(feature = "controller")]
#[path = "player.rs"]
pub mod player;

#[cfg(feature = "deemix")]
pub mod deemix;

pub mod events;
pub mod models;
pub mod controller;
pub mod radio;
pub mod compat;
pub mod usersettoken;

// #[cfg(feature = "http-get")]
// pub mod httpget;

#[cfg(feature = "check")]
pub mod check;

#[cfg(test)]
mod testsuite;

use serenity::client::ClientBuilder;
pub async fn init(mut cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("Mockingbird initializing...");
    use songbird::SerenityInit;


    #[cfg(feature = "controller")]
    {
        use std::collections::HashMap;
        cfg = cfg.type_map_insert::<models::LazyQueueKey>(HashMap::new());
    }

    cfg.register_songbird()
}
