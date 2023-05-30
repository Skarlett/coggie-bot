
mod controller;
mod deemix;

use serenity::client::ClientBuilder;

#[cfg(feature="controls")]
pub use controller::COMMANDS_GROUP;

pub async fn init(cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("Mockingbird initializing...");
    use songbird::SerenityInit;
    cfg.register_songbird()
}

#[cfg(test)]
mod testsuite;