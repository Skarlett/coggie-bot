//! Example demonstrating how to make use of individual track audio events,
//! and how to use the `TrackQueue` system.
//!
//! Requires the "cache", "standard_framework", and "voice" features be enabled in your
//! Cargo.toml, like so:
//!
//! ```toml
//! [dependencies.serenity]
//! git = "https://github.com/serenity-rs/serenity.git"
//! features = ["cache", "framework", "standard_framework", "voice"]
//! ```
//use super::lib::ArlToken;
//
use serenity::client::ClientBuilder;

mod extractor;
pub mod controller;

pub async fn init(cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("Mockingbird initialized");
    return extractor::init(cfg).await;
}

#[cfg(test)]
mod tests {

    use std::env::var;
    use std::path::PathBuf;

    #[test]
    fn path_ffmpeg() {
        let paths = var("PATH").unwrap();
        assert!(paths.split(':').filter(|p| PathBuf::from(p).join("ffmpeg").exists()).count() >= 1);
    }

    #[cfg(feature="mockingbird-ytdl")]
    #[test]
    fn path_ytdl() {
        let paths = var("PATH").unwrap();
        assert!(paths.split(':').filter(|p| PathBuf::from(p).join("yt-dlp").exists()).count() == 1);
    }

    #[cfg(feature="mockingbird-deemix")]
    #[test]
    fn path_deemix() {
        let paths = var("PATH").unwrap();
        assert!(paths.split(':').filter(|p| PathBuf::from(p).join("deemix").exists()).count() == 1);
    }
}
