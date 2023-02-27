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
use super::ArlToken;

use std::{
    sync::Arc,
    time::Duration,
};

use std::{
    io::{BufRead, BufReader, Read},
    process::{Command, Stdio},
};
use tokio::{process::Command as TokioCommand, task};

use std::ffi::OsStr;
use serenity::{
    async_trait,
    client::Context,
    framework::{
        standard::{
            macros::{command, group},
            Args,
            CommandResult,
        },
    },
    http::Http,
    model::{channel::Message, prelude::ChannelId},
    prelude::Mentionable,
    Result as SerenityResult,
};

use songbird::{
    input::{
        self,
        restartable::{Restartable, Restart},
        Input,
        Container,
        Codec,
        Metadata,
        error::Error as InputError,
        children_to_reader,
    },
    Event,
    EventContext,
    EventHandler as VoiceEventHandler,
    TrackEvent,
};

pub struct MockingbirdConfig {
    #[cfg(feature="demix")]
    demix: DemixConfig,
}

#[group]
#[commands(
    deafen, join, leave, mute, play_fade, queue, skip, stop, undeafen, unmute
)]
struct MockingBird;
struct DeezerRestarter<P>
{
    uri: P,
    arl: String
}

fn deezer(uri: &str, arl: &str, pre_args: &[&str]) -> Result<Input, InputError>
{
    let demix_args = [
        "--arl", arl,
        "-b", "128000",
        uri,
    ];

    let ffmpeg_args = [
        "-f",
        "s16le",
        "-ac",
        "2",
        "-ar",
        "48000",
        "-acodec",
        "pcm_f32le",
        "-",
    ];

    let mut demix_pipe = Command::new("pipe_demix")
        .args(&demix_args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let taken_stdout = demix_pipe.stdout.take().ok_or(InputError::Stdout)?;
    let ffmpeg = Command::new("ffmpeg")
        .args(pre_args)
        .arg("-i")
        .arg("-")
        .args(&ffmpeg_args)
        .stdin(taken_stdout)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;

    //let metadata = Metadata::from_ytdl_output(value?);

    Ok(Input::new(
        true,
        children_to_reader::<f32>(vec![demix_pipe, ffmpeg]),
        Codec::FloatPcm,
        Container::Raw,
        None,
    ))
}

#[async_trait]
impl Restart for DeezerRestarter<String>
{
    async fn call_restart(&mut self, time: Option<Duration>) -> Result<Input, InputError>
    {
        deezer(&self.uri, &self.arl, &[])
    }
    async fn lazy_init(&mut self) -> Result<(Option<Metadata>, Codec, Container), InputError>
    {
        //deezer(&self.uri, &self.arl, &["-ss", "0:00:05"])
        todo!()
    }
}

pub async fn on_dj_channel(ctx: &Context, msg: &Message) -> CommandResult {
    let url = match &msg.content
    {
        url if url.starts_with("http") => url.to_string(),
        _ => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "Must provide a URL to a video or audio")
                    .await,
            );

            return Ok(());
        },
    };

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        if url.contains("deezer.page")
        {
            let arl = match ctx.data.read().await.get::<ArlToken>() {
                Some(arl) => arl.clone(),
                None => {
                    check_msg(
                        msg.channel_id
                            .say(&ctx.http, "No ARL token found")
                            .await,
                    );

                    return Ok(());
                }
            };

            let restarter = match deezer(&url, &arl, &[] ){
                Ok(src) => src,
                Err(e) => {
                    check_msg(
                        msg.channel_id
                            .say(&ctx.http, format!("Error: {}", e))
                            .await,
                    );
                    return Ok(());
                }
            };

            handler.enqueue_source(restarter.into());
        }
        else {
            // Here, we use lazy restartable sources to make sure that we don't pay
            // for decoding, playback on tracks which aren't actually live yet.
            let source = match Restartable::ytdl(url, true).await {
                Ok(source) => source,
                Err(why) => {
                    println!("Err starting source: {:?}", why);
                    check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

                    return Ok(());
                },
            };
            handler.enqueue_source(source.into());
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

#[command("arl")]
async fn get_arl(ctx: &Context, msg: &Message) -> CommandResult {

    let arl = ctx.data.read().await.get::<ArlToken>().expect("Expected CommandCounter in TypeMap.").clone();
    msg.channel_id.say(&ctx.http, arl).await?;
    Ok(())
}

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

#[command]
#[only_in(guilds)]
async fn play_fade(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
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

        let source = match input::ytdl(&url).await {
            Ok(source) => source,
            Err(why) => {
                println!("Err starting source: {:?}", why);

                check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

                return Ok(());
            },
        };

        // This handler object will allow you to, as needed,
        // control the audio track via events and further commands.
        let song = handler.play_source(source);
        let send_http = ctx.http.clone();
        let chan_id = msg.channel_id;

        // This shows how to periodically fire an event, in this case to
        // periodically make a track quieter until it can be no longer heard.
        let _ = song.add_event(
            Event::Periodic(Duration::from_secs(5), Some(Duration::from_secs(7))),
            SongFader {
                chan_id,
                http: send_http,
            },
        );

        let send_http = ctx.http.clone();

        // This shows how to fire an event once an audio track completes,
        // either due to hitting the end of the bytestream or stopped by user code.
        let _ = song.add_event(
            Event::Track(TrackEvent::End),
            SongEndNotifier {
                chan_id,
                http: send_http,
            },
        );

        check_msg(msg.channel_id.say(&ctx.http, "Playing song").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}

struct SongFader {
    chan_id: ChannelId,
    http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for SongFader {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(&[(state, track)]) = ctx {
            let _ = track.set_volume(state.volume / 2.0);

            if state.volume < 1e-2 {
                let _ = track.stop();
                check_msg(self.chan_id.say(&self.http, "Stopping song...").await);
                Some(Event::Cancel)
            } else {
                check_msg(self.chan_id.say(&self.http, "Volume reduced.").await);
                None
            }
        } else {
            None
        }
    }
}

struct SongEndNotifier {
    chan_id: ChannelId,
    http: Arc<Http>,
}

#[async_trait]
impl VoiceEventHandler for SongEndNotifier {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        check_msg(
            self.chan_id
                .say(&self.http, "Song faded out completely!")
                .await,
        );

        None
    }
}

#[command]
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

        // Here, we use lazy restartable sources to make sure that we don't pay
        // for decoding, playback on tracks which aren't actually live yet.
        let source = match Restartable::ytdl(url, true).await {
            Ok(source) => source,
            Err(why) => {
                println!("Err starting source: {:?}", why);

                check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

                return Ok(());
            },
        };

        handler.enqueue_source(source.into());

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

/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
