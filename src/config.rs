use serde::{Deserialize, Serialize};



#[derive(Debug, Default)]
struct Bookmark {
    emote: String,
}


#[derive(Debug, Default)]
pub struct Configuration {
    prefixes: Vec<String>,
    repo: String,
    commit: String,

    maintainers: Vec<Maintainer>,


    #[cfg(feature = "bookmark-emoji")]
    bookmark: Bookmark,

    #[cfg(feature = "dj-room")]
    dj_room: Vec<u64>,

    #[cfg(feature="list-feature-cmd")]
    features: Vec<Feature>,
}

#[derive(Debug, Deserialize)]
struct Maintainer {
    discordid: u64,
    github: Option<String>,
    languages : Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Feature {
    name: String,
    dependencies: Vec<String>,
    enabled: bool,
    maintainers: Vec<String>,
}

struct Command {
    aliases: Vec<String>,
    description: String,
    example: String,
    owner_only: bool,
    hidden: bool,
    guild_only: bool,
    dm_only: bool,
    nsfw: bool,
    cooldown: u64,
    sub_commands: Vec<Command>,
    config: serde_json::Value,
}
