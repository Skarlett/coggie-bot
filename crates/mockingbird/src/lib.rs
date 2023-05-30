
mod controller;
mod deemix;

#[cfg(test)]
mod testsuite;

pub use controller::COMMANDS_GROUP as COMMANDS;

use serenity::client::ClientBuilder;
pub async fn init(cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("Mockingbird initializing...");
    use songbird::SerenityInit;
    cfg.register_songbird()
}
