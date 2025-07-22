use serenity::model::channel::ChannelType;
use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    model::{
        channel::Message,
        gateway::Ready,
        voice::VoiceState,
        id::{ChannelId, MessageId, GuildId},
    },
    prelude::*,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    fs,
    sync::Arc,
};
use tokio::{
    sync::RwLock,
    time::{sleep, Duration},
};
use serenity::client::ClientBuilder;

// Configuration structure that matches the JSON format
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotifyConfig {
    #[serde(flatten)]
    pub guilds: HashMap<String, HashMap<String, String>>,
}

impl NotifyConfig {
    fn new() -> Self {
        Self { guilds: HashMap::new() }
    }
}

pub struct VcActionKey;
impl TypeMapKey for VcActionKey {
    type Value = Arc<Mutex<VcAction>>;
}

pub async fn init(mut cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("VcNotify initializing...");
    cfg =
        cfg
        .type_map_insert::<VcActionKey>(
            Arc::new(Mutex::new(VcAction { config: {
                let content = match fs::read_to_string(
                    std::env::var("COG_NOTIFY_MAP")
                        .unwrap_or("/etc/coggiebot/vc-notify.json".to_string())
                ) {
                    Ok(x) => x,
                    Err(e) => {
                        //TODO: tracing::error!(e);
                        String::new()
                    }
                };

                serde_json::from_str(&content).unwrap()
            }})))
    ;

    cfg
}

pub struct VcAction {
    config: NotifyConfig,
}

impl VcAction {
    pub async fn get_text_channel(&self, guild_id: GuildId, voice_channel_id: ChannelId) -> Option<ChannelId> {
        let guild_str = guild_id.to_string();
        let voice_str = voice_channel_id.to_string();

        self.config
            .guilds
            .get(&guild_str)?
            .get(&voice_str)
            .and_then(|text_str| text_str.parse::<u64>().ok())
            .map(ChannelId)
    }

    pub async fn set_channel_pair(&mut self, guild_id: GuildId, voice_channel_id: ChannelId, text_channel_id: ChannelId) {
        let guild_str = guild_id.to_string();
        let voice_str = voice_channel_id.to_string();
        let text_str = text_channel_id.to_string();

        self.config
            .guilds
            .entry(guild_str)
            .or_insert_with(HashMap::new)
            .insert(voice_str, text_str);
    }

    pub async fn remove_channel_pair(&mut self, guild_id: GuildId, voice_channel_id: ChannelId) -> bool {
        let guild_str = guild_id.to_string();
        let voice_str = voice_channel_id.to_string();

        if let Some(guild_map) = self.config.guilds.get_mut(&guild_str) {
            guild_map.remove(&voice_str).is_some()
        } else {
            false
        }
    }

    pub async fn list_pairs_for_guild(&self, guild_id: GuildId) -> HashMap<ChannelId, ChannelId> {
        let guild_str = guild_id.to_string();

        self.config
            .guilds
            .get(&guild_str)
            .map(|guild_map| {
                guild_map
                    .iter()
                    .filter_map(|(voice_str, text_str)| {
                        let voice_id = voice_str.parse::<u64>().ok().map(ChannelId)?;
                        let text_id = text_str.parse::<u64>().ok().map(ChannelId)?;
                        Some((voice_id, text_id))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub async fn clear_guild(&mut self, guild_id: GuildId) -> bool {
        let guild_str = guild_id.to_string();
        self.config.guilds.remove(&guild_str).is_some()
    }
}

pub async fn handle_voice_join(
    ctx: &Context,
    voice_state: &VoiceState,
    voice_channel_id: ChannelId,
    config_manager: &VcAction,
) {
    if let Some(user) = &voice_state.member {
        if let Some(guild_id) = voice_state.guild_id {
            let username = &user.user.name;

            if let Some(text_channel_id) = config_manager.get_text_channel(guild_id, voice_channel_id).await {
                let message = format!("🔊 **{}** joined the voice channel", username);

                if let Ok(sent_message) = text_channel_id.say(&ctx.http, &message).await {
                    // Schedule message deletion after 60 seconds
                    schedule_message_deletion(ctx.clone(), text_channel_id, sent_message.id).await;
                }
            }
        }
    }
}

pub async fn handle_voice_leave(
    ctx: &Context,
    voice_state: &VoiceState,
    voice_channel_id: ChannelId,
    config_manager: &VcAction,
) {
    if let Some(user) = &voice_state.member {
        if let Some(guild_id) = voice_state.guild_id {
            let username = &user.user.name;

            if let Some(text_channel_id) = config_manager.get_text_channel(guild_id, voice_channel_id).await {
                let message = format!("🔇 **{}** left the voice channel", username);

                if let Ok(sent_message) = text_channel_id.say(&ctx.http, &message).await {
                    // Schedule message deletion after 60 seconds
                    schedule_message_deletion(ctx.clone(), text_channel_id, sent_message.id).await;
                }
            }
        }
    }
}

async fn schedule_message_deletion(ctx: Context, channel_id: ChannelId, message_id: MessageId) {
    tokio::spawn(async move {
        // Wait 60 seconds
        sleep(Duration::from_secs(60)).await;

        // Delete the message
        if let Err(e) = ctx.http.delete_message(channel_id.0, message_id.0).await {
            eprintln!("Failed to delete message: {:?}", e);
        }
    });
}

// TODO: experiment and see if voice channels are
// marked as parents to the text channels
//------------------------------------------------
// async fn setup_room(x: Context, guild_id: u64) -> Vec<ChannelId> {
//     let mut channels = x.cache.guild_channels(guild_id).unwrap();

//     let parents = channels.filter(|x| {
//         if let ChannelType::Voice = ch.kind {
//             return true;
//         }
//         false
//     }).map(|x| x.id).collect();

//     channels.retain(|x| parents.contains(x.parent_id));
//     return channels.map(|x| x.id).collect()
// }
