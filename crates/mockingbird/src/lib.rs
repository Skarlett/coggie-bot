#[cfg(feature = "standard-controller")]
mod controller;


#[cfg(feature = "standard-controller")]
pub use controller::COMMANDS_GROUP as COMMANDS;
#[cfg(feature = "beta-controller")]
#[path = "player.rs"]
pub mod player;

#[cfg(feature = "deemix")]
mod deemix;

#[cfg(feature = "check")]
pub mod check;

#[cfg(test)]
mod testsuite;


use serenity::client::ClientBuilder;


pub async fn init(mut cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("Mockingbird initializing...");
    use songbird::SerenityInit;


    #[cfg(feature = "beta-controller")]
    {
        use std::collections::HashMap;
        cfg = cfg.type_map_insert::<player::LazyQueueKey>(HashMap::new());
    }

    cfg.register_songbird()
}
