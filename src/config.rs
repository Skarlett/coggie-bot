use confique::Config;
use serde::{Deserialize, Serialize};


#[cfg(feature = "bookmark-emoji")]
#[derive(Debug, Config)]
pub struct BookmarkConfig {
    bookmark_emoji: String,
}

#[cfg(feature = "dj-room")]
#[derive(Debug, Config, Serialize, Deserialize)]
pub struct DjRoom {
    text_channel_id: u64,
    voice_channel_id: u64,
}

#[allow(dead_code)]
#[derive(Debug, Config, Default)]
pub struct Configuration {
    prefixes: Vec<String>,

    #[config(env = "DISCORD_TOKEN")]
    token: String,

    #[cfg(feature = "bookmark-emoji")]
    #[config(default = "\u{1F516}")]
    bookmark_emoji: String,

    #[cfg(feature = "list-feature-cmd")]
    #[config(env = "COGGIEBOT_FEATURE_FILE")]
    feature_file: String,

    #[cfg(feature = "dj-room")]
    dj_room: Vec<DjRoom>,
}
