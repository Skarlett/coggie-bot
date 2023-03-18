use serde::{Deserialize, Serialize};


#[derive(Debug, Default)]
pub struct Configuration {
    prefixes: Vec<String>,

    #[cfg(feature = "bookmark-emoji")]
    #[config(default = "\u{1F516}")]
    bookmark_emoji: String,

    #[cfg(feature = "dj-room")]
    dj_room: Vec<u64>,

    #[cfg(feature="list-feature-cmd")]
    features: HashMap<String, u8>,
}
