use songbird::{
    EventHandler as VoiceEventHandler,
    EventContext,
    TrackEvent,
    Event as VCEvent,
    input,
    tracks::{create_player, TrackHandle}
};
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, Args,
};

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::time::Duration;
use std::io::SeekFrom as Seek;

use super::extractor::{play_source, PlaySource};

#[cfg(feature="mockingbird-deemix")]
use super::extractor::{DxConfigKey, DxConfig};


#[group]
#[commands(
    deafen, join, leave, mute, skip, stop, undeafen, unmute, queue
)]
struct Deemix;


#[command]
async fn deafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    let mut handler = handler_lock.lock().await;
    if handler.is_deaf() {
        check_msg(msg.channel_id.say(&ctx.http, "Already deafened").await);
    } else {
        if let Err(e) = handler.deafen(true).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Deafened").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let (_handle_lock, success) = manager.join(guild_id, connect_to).await;

    let reply = match success {
        Ok(_) => format!("Joined {}", connect_to.mention()),
        Err(e) =>format!("Failed to join voice channel: {:?}", e),
    };

    check_msg(
        msg.channel_id
           .say(&ctx.http, reply)
           .await
    );

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn mute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    let mut handler = handler_lock.lock().await;

    if handler.is_mute() {
        check_msg(msg.channel_id.say(&ctx.http, "Already muted").await);
    } else {
        if let Err(e) = handler.mute(true).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Now muted").await);
    }

    Ok(())
}

struct HardDelete(Option<PathBuf>);
#[async_trait]
impl VoiceEventHandler for HardDelete {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<VCEvent> {
        if let EventContext::Track(&[(a, track_list)]) = ctx {
            if let Some(ref path) = self.0 {
                tracing::info!("Deleting {:?}", path);
                tokio::fs::remove_file(path).await.unwrap();
            }
        }
        None
    }
}

// struct FadeOver { 
//     next: TrackHandle,
//     next_playing: Arc<AtomicUsize>,
//     songbird: Arc<Mutex<Call>>
// }

// #[async_trait]
// impl VoiceEventHandler for FadeOver {
//     async fn act(&self, ctx: &EventContext<'_>) -> Option<VCEvent> {
//         if let EventContext::Track(&[(state, track)]) = ctx {
//             let first = self.next_playing.fetch_add(1, Ordering::Relaxed); 
//             if first == 0 {
//                 let handler = self.songbird.lock().await?;
//                 let mut handler = self.songbird.get(self.next.guild_id).unwrap().lock().await;

//                 self.next.make_playable();
//                 self.next.seek_time(Duration::from_secs(0));
                
                
//                 tracing::info!("Playing next track");
//             }

//             if state.volume < 0.2 {
//                 tracing::info!("Track volume is low, cancelling");
//                 track.stop();
//                 self.songbird.lock().await?;
//                 handler.play(self.next);
//                 self.next.set_volume(1.0);
//                 self.next.seek_time(Duration::from_secs(9));
//                 return Some(VCEvent::Cancel)
//             }
//             else {
//                 track.set_volume(state.volume - 0.1);
//                 self.next.set_volume(1.0 - state.volume);
//             }
//         }
//         None
//     }
// }

#[command]
#[aliases("play")]
#[only_in(guilds)]
async fn queue(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "Must provide a URL to a video or audio")
                    .await,
            );

            return Ok(());
        },
    };

    if !url.starts_with("http") {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Must provide a valid URL")
                .await,
        );

        return Ok(());
    }

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();


    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let req = super::extractor::PlayRequest {
            uri: &url,
            #[cfg(feature = "mockingbird-deemix")]
            dx: {
                let dx_lock = ctx.data.read().await;
                let dxc: DxConfig = dx_lock.get::<DxConfigKey>().unwrap().clone();
                dxc
            }
        };

        match play_source(req).await.unwrap() {
            PlaySource::Ytdl { uri } => {
                let input = input::ytdl(&uri).await.unwrap();              
                handler.enqueue_source(input.into());
            }

            PlaySource::FileSystem { errlog, ok_paths } => {
                for fp in ok_paths {
                    match input::ffmpeg(&fp).await
                    {
                        Ok(input) => {
                            let (track, track_handle) = create_player(input);
                            
                            // if let Some(x) = handler.queue().current_queue().first() {
                            //     track_handle.add_event(
                            //         VCEvent::Periodic(Duration::from_secs(1), Some(track_handle.metadata().duration.unwrap() - Duration::from_secs(5))),
                            //         FadeOver {
                            //             next: x.clone(),
                            //             next_playing: Arc::new(AtomicUsize::new(0)),
                            //         }
                            //     );
                            // }
                            #[cfg(feature="mockingbird-hard-cleanfs")]
                            track_handle.add_event(
                                VCEvent::Track(TrackEvent::End),
                                HardDelete(Some(fp))
                            );
                            handler.enqueue(track);
                        },
                        
                        Err(e) => {
                            tokio::fs::remove_file(&fp).await.unwrap();
                            continue;
                        }
                    }
                }                
            }
        } 

        check_msg(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("Added song to queue: position {}", handler.queue().len()),
                )
                .await,
        );
    }
    else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.skip();

        check_msg(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("Song skipped: {} in queue.",
                        queue.len()
                    ),
                )
                .await,
        );
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.stop();
        check_msg(msg.channel_id.say(&ctx.http, "Queue cleared.").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn undeafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.deafen(false).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Undeafened").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to undeafen in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn unmute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Unmuted").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to unmute in")
                .await,
        );
    }

    Ok(())
}

use serenity::Result as SerenityResult;
/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
