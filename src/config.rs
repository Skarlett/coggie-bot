use confique::Config;

#[cfg(feature = "bookmark-emoji")]
#[derive(Debug, Config)]
struct BookmarkConfig {
    bookmark_emoji: String,
}

#[cfg(feature = "dj-room")]
#[derive(Debug, Config)]
struct DjRoom {
    text_channel_id: u64,
    voice_channel_id: u64,
}

#[derive(Debug, Config, Default)]
pub struct Configuration {
    #[config(env = "DISCORD_TOKEN")]
    token: String,
    prefixes: Vec<String>,

    #[cfg(feature = "bookmark-emoji")]
    #[config(default = "\u{1F516}")]
    bookmark_emoji: String,

    #[cfg(feature = "list-feature-cmd")]
    #[config(env = "COGGIEBOT_FEATURE_FILE")]
    feature_file: String,

    #[cfg(feature = "mockingbird")]
    mockingbird: crate::controllers::mockingbird::MockingbirdConfig,

    #[cfg(feature = "dj-room")]
    #[config(path = "dj-room")]
    dj_room: Vec<DjRoom>,
}
