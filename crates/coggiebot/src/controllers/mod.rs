mod vc_notify;

#[cfg(feature = "llm")]
#[path = "llm.rs"]
mod llm;

#[cfg(feature = "bookmark")]
#[path = "bookmark.rs"]
mod bookmark;

#[cfg(feature = "basic-cmds")]
#[path = "basic.rs"]
mod basic;

#[cfg(feature = "list-feature-cmd")]
#[path = "features.rs"]
pub mod features;

#[cfg(feature = "prerelease")]
#[path = "prerelease.rs"]
pub mod prerelease;

use serenity::async_trait;
use serenity::{framework::StandardFramework, client::ClientBuilder};
use serenity::model::{channel::Reaction, gateway::Ready};
use serenity::prelude::*;

macro_rules! add_commands {
    ($framework:expr, { $( [ $($feature:literal),* ] => [ $($group:expr),* ]),* })
        => {
            $(#[cfg(all( $(feature = $feature),* ))]
              { $framework = $framework$(.group(&$group))*; })*
        }
}

#[allow(unused_mut)]
pub fn setup_framework(mut cfg: StandardFramework) -> StandardFramework {
    add_commands!(
        cfg,
        {
            ["basic-cmds"] => [basic::COMMANDS_GROUP],
            ["list-feature-cmd"] => [features::FEATURES_GROUP],
            ["llm"] => [ llm::LLMCOMMANDS_GROUP ],
            //TODO: ["prerelease"] => [features::PRERELEASE_GROUP::PRERELEASE_GROUP],
            //TODO: ["help-cmd"] => [features::HELP_GROUP],
            ["mockingbird-set-arl-cmd"] => [mockingbird::usersettoken::DANGEROUS_GROUP],
            ["mockingbird-ctrl"] => [mockingbird::controller::BETTERPLAYER_GROUP],
            ["mockingbird-ctrl", "mockingbird-radio"] => [mockingbird::radio::RADIO_GROUP],
            ["mockingbird-ctrl", "mockingbird-crossfade"] => [mockingbird::crossfade::CROSSFADE_GROUP]
        }
    );

    #[cfg(feature = "llm")]
    { cfg = llm::setup_framework(cfg); }

    cfg
}


#[allow(unused_mut)]
pub async fn setup_state(mut cfg: ClientBuilder) -> ClientBuilder {
    #[cfg(feature = "mockingbird-core")]
    {
        use mockingbird::init as mockingbird_init;
        cfg = mockingbird_init(cfg).await;
    }

    #[cfg(feature = "llm")]
    {
        cfg = llm::init(cfg).await;
    }

    #[cfg(feature = "vc-notify")]
    {
        cfg = vc_notify::init(cfg).await;
    }

    cfg
}


pub struct EvHandler;


use serenity::model::voice::VoiceState;
use crate::controllers::vc_notify::VcActionKey;

#[async_trait]
impl EventHandler for EvHandler {
    #[allow(unused_variables)]
    async fn reaction_add(&self, ctx: Context, ev: Reaction) {

        #[cfg(feature="bookmark")]
        tokio::spawn(async move {
            use bookmark::bookmark_on_react_add;
            match bookmark_on_react_add(&ctx, &ev).await {
                Ok(_) => {},
                Err(e) => { ev.channel_id.say(&ctx.http, format!("Error: {}", e)).await.unwrap(); },
            };
        });
    }

    #[cfg(feature="vc-notify")]
    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        // Handle voice channel join

        let data = ctx.data.read().await;
        let x = data.get::<VcActionKey>().unwrap().clone();

        if old.as_ref().map(|o| o.channel_id).is_none() && new.channel_id.is_some() {
            if let Some(channel_id) = new.channel_id {
                let cfg_mgr = x.lock().await;
                vc_notify::handle_voice_join(&ctx, &new, channel_id, &cfg_mgr).await;
            }
        }

        // Handle voice channel leave
        if let Some(old_state) = &old {
            if old_state.channel_id.is_some() && new.channel_id.is_none() {
                if let Some(channel_id) = old_state.channel_id {
                   let cfg_mgr = x.lock().await;
                   vc_notify::handle_voice_leave(&ctx, &new, channel_id, &cfg_mgr).await;
                }
            }
        }

        // Handle voice channel move (leave old, join new)
        if let Some(old_state) = &old {
            if let (Some(old_channel), Some(new_channel)) = (old_state.channel_id, new.channel_id) {
                if old_channel != new_channel {
                   let cfg_mgr = x.lock().await;
                    vc_notify::handle_voice_leave(&ctx, &new, old_channel, &cfg_mgr).await;
                    vc_notify::handle_voice_join(&ctx, &new, new_channel, &cfg_mgr).await;
                }
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}
