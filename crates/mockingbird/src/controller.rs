use crate::{models::*, player::play};

use serenity::{
    framework::standard::{
        macros::{command, group}, Args, CommandResult
    },
    model::{channel::Message, prelude::*},
    prelude::*
};

use songbird::{
    error::JoinError,
    input::Metadata,
};

use std::{
    time::Instant,
    sync::Arc,
};

use core::sync::atomic::Ordering;
use crate::player::join_routine;
use crate::player::next_track_handle;

#[group]
#[commands(join, leave, queue, now_playing, skip, list, shuffle)]
pub struct BetterPlayer;

#[command]
#[aliases("np", "playing", "now-playing", "playing-now", "nowplaying")]
#[only_in(guilds)]
async fn now_playing(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    tracing::info!(
        "[{}::{}] asked what track is playing in [{}::{:?}]",
        msg.author.id, msg.author.name,
        msg.channel_id, msg.channel_id.name(&ctx).await
   );


    let qctx = {
        let mut glob = ctx.data.write().await;
        let queue = glob.get_mut::<LazyQueueKey>()
            .expect("Expected LazyQueueKey in TypeMap");
        queue.get(&guild_id).cloned()
    };

    let qctx = match qctx {
        Some(qctx) => qctx,
        None => {
            msg.channel_id
               .say(&ctx.http, "Not in a voice channel")
               .await?;
            return Ok(());
        }
    };

    let call_lock = qctx.manager
        .get(qctx.guild_id)
        .unwrap();

    let call = call_lock.lock().await;

    match call.queue().current() {
        Some(ref x) => {
            msg.channel_id
               .say(&ctx.http,
                    format!(
                        "{}: {}", qctx.voice_chan_id.mention(),
                        x.metadata()
                            .clone()
                            .source_url
                            .unwrap_or("Unknown".to_string())
                    )
               ).await?;
        }
        None => {
            msg.channel_id
               .say(&ctx.http, "Nothing is currently playing")
               .await?;
        }
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let connect_to = crate::player::join_routine(&ctx, msg).await;

    if let Err(ref e) = connect_to {
        msg.channel_id
           .say(&ctx.http, format!("Failed to join voice channel: {:?}", e))
           .await?;
    }

    msg.channel_id
       .say(&ctx.http, format!("Joined {}", connect_to.unwrap().voice_chan_id.mention()))
       .await?;

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("songbird voice client placed in at initialisation.")
        .clone();

    let handler = manager.get(guild_id);

    if handler.is_none() {
        msg.reply(ctx, "Not in a voice channel").await?;
        return Ok(())
    }

    let handler = handler.unwrap();

    {
        let mut call = handler.lock().await;
        call.remove_all_global_events();
        call.stop();
        let _ = call.deafen(false).await;
    }

    if let Err(e) = manager.remove(guild_id).await {
        msg.channel_id
           .say(&ctx.http, format!("Failed: {:?}", e))
           .await?;
    }

    {
        let mut glob = ctx.data.write().await;
        let queue = glob.get_mut::<LazyQueueKey>().expect("Expected LazyQueueKey in TypeMap");
        queue.remove(&guild_id);
    }

    msg.channel_id.say(&ctx.http, "Left voice channel").await?;
    Ok(())
}

#[command]
#[aliases("play", "p", "q")]
#[only_in(guilds)]
async fn queue(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    tracing::info!(
        "[{}::{}] queued track in [{}::{:?}]",
        msg.author.id, msg.author.name,
        msg.channel_id, msg.channel_id.name(&ctx).await
    );

    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            msg.channel_id
               .say(&ctx.http, "Must provide a URL to a video or audio")
               .await
               .unwrap();
            return Ok(());
        },
    };

    if !url.starts_with("http") {
        msg.channel_id
           .say(&ctx.http, "Must provide a valid URL")
           .await
           .unwrap();
        return Ok(());
    };

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let qctx: Arc<QueueContext>;

    // grab the call object from guild ID.
    let call = match manager.get(guild_id) {
        Some(call_lock) => {
            qctx = ctx.data.write()
                .await
                .get_mut::<LazyQueueKey>()
                .unwrap()
                .get_mut(&guild_id)
                .unwrap()
                .clone();

            call_lock
        },

        None => {
            // Join the VC the user is in,
            // then try again.
            let tmp = crate::player::join_routine(ctx, msg).await;

            if let Err(ref e) = tmp {
                msg.channel_id
                   .say(&ctx.http, format!("Failed to join voice channel: {:?}", e))
                   .await
                   .unwrap();
                return Ok(());
            };
            qctx = tmp.unwrap();
            msg.channel_id
                   .say(&ctx.http, format!("Joined: {}", qctx.voice_chan_id.mention()))
                   .await
                   .unwrap();

            let call = manager.get(guild_id).ok_or_else(|| JoinError::NoCall);
            call?
        }
    };

    match crate::player::Players::from_str(&url)
        .ok_or_else(|| String::from("Failed to select extractor for URL"))
    {
        Ok(player) => {
            let mut uris = player.fan_collection(url.as_str()).await?;
            let added = uris.len();

            // YTDLP singles don't work.
            // so instead, use the original URI.
            if uris.len() == 1 && player == crate::player::Players::Ytdl {
                uris.clear();
                uris.push_back(url.clone());
            }

            // --- START
            // WARNING: removing these curly braces will cause a deadlock.
            // amount of hours spent on this: 5
            {
                let crossfading = qctx.crossfade.load(Ordering::Relaxed);
                qctx.cold_queue.write().await.queue.extend(uris.drain(..));

                // check for hot loaded track
                let hot_loaded = {
                    let call = call.lock().await;
                    if crossfading { 
                        let lock = qctx.cold_queue.write().await; 
                        lock.crossfade_rhs.is_some() || lock.crossfade_lhs.is_some() 
                    } 
                    else { call.queue().len() > 0 }
                };

                if hot_loaded == false {
                    let mut call = call.lock().await;
                    let mut cold_queue = qctx.cold_queue.write().await;
                    let next = crate::player::next_track_handle(&mut cold_queue, qctx.clone(), crossfading).await;

                    if let Ok(Some((track, handle, _metadata))) = next {
                        play(&mut call, track, &handle, &mut cold_queue, crossfading).await?;
                    }
                }
            }
            // --- END


            let content = format!(
                "Added {} Song(s) [{}] queued",
                added,
                qctx.cold_queue.read().await.queue.len()
            );

            msg.channel_id
               .say(&ctx.http, &content)
               .await?;
        },

        Err(_) => {
            msg.channel_id
               .say(&ctx.http, format!("Failed to select extractor for URL: {}", url))
               .await?;
        }
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    tracing::info!(
        "[{}::{}] skipped track in [{}::{:?}]",
        msg.author.id, msg.author.name,
        msg.channel_id, msg.channel_id.name(&ctx).await
    );

    let qctx = ctx.data.write().await
        .get_mut::<LazyQueueKey>().unwrap()
        .get_mut(&guild_id).unwrap().clone();

    let cold_queue_len = qctx.cold_queue.read().await.queue.len();

    let skipn = args.remains()
        .unwrap_or("1")
        .parse::<isize>()
        .unwrap_or(1);

    // stop_event: EventEnd::UnMarked,

    if 1 > skipn  {
        msg.channel_id
           .say(&ctx.http, "Must skip at least 1 song")
           .await?;
        return Ok(())
    }

    else if skipn >= cold_queue_len as isize + 1 {
        qctx.cold_queue.write().await.queue.clear();
    }

    else {
        let mut cold_queue = qctx.cold_queue.write().await;
        let bottom = cold_queue.queue.split_off(skipn as usize - 1);
        cold_queue.queue.clear();
        cold_queue.queue.extend(bottom);
    }

    // --- START
    // stand alone section, writes historical actions.
    {
        let mut cold_queue = qctx.cold_queue.write().await;
        if let Some(x) = cold_queue.has_played.front_mut()
        {
            if let EventEnd::UnMarked = x.stop_event
            {
                x.stop_event = EventEnd::Skipped;
                x.end = Instant::now();
            }
        }
    }
    // -- END

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let crossfading = qctx.crossfade.load(Ordering::Relaxed);

    {
        match manager.get(guild_id) {
            Some(call) => {
                let call = call.lock().await;
                if ! crossfading {
                    let queue = call.queue();
                    let _ = queue.skip();
                    return Ok(());
                };
            }
            None => {
                msg.channel_id
                    .say(&ctx.http, "Not in a voice channel to play in")
                    .await?;
                return Ok(());
            }
        };

        let mut cold_queue = qctx.cold_queue.write().await;
        match (cold_queue.crossfade_lhs.take(), cold_queue.crossfade_rhs.take()) {
            (None, None) => {}

            (Some(lhs), Some(rhs)) => {
                let _ = lhs.stop();
                cold_queue.crossfade_lhs = Some(rhs);
                cold_queue.crossfade_rhs = None;
            }

            (Some(lhs), None) => {
                let _ = lhs.stop();
                cold_queue.crossfade_lhs = None;

                match manager.get(guild_id) {
                    Some(call) => {
                        let mut call = call.lock().await;
                        if let Ok(Some( (mut track, handle, metadata) )) = next_track_handle(&mut cold_queue, qctx.clone(), true).await {
                            play(&mut call, track, &handle, &mut cold_queue, true).await?;
                        }
                    }
                    None => {
                        msg.channel_id
                            .say(&ctx.http, "Not in a voice channel to play in")
                            .await?;
                        return Ok(());
                    }
                };
            }

            (None, Some(rhs)) => {
                cold_queue.crossfade_lhs = Some(rhs);
                cold_queue.crossfade_rhs = None;
            }
        }
    }

    let reply_msg = format!(
        "Song skipped [{}]: {} in queue.",
        skipn,
        skipn-cold_queue_len as isize
    );

    msg.reply(
        &ctx,
        reply_msg,
    ).await?;

    Ok(())
}

#[command]
#[only_in(guilds)]
#[aliases("ls", "l")]
/// @bot list
async fn list(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let mut _qctx_lock = ctx.data.write().await;
    let mut _qctx = _qctx_lock
        .get_mut::<LazyQueueKey>()
        .expect("Expected LazyQueueKey in TypeMap");

    if let None = _qctx.get(&guild_id) {
        msg.channel_id
           .say(&ctx.http, "Not in a voice channel")
           .await?;
        return Ok(())
    }
    let qctx = _qctx.get_mut(&guild_id).unwrap();
    let cold_queue = qctx.cold_queue.read().await;

    msg.channel_id
       .say(&ctx.http,
            format!(
                "{}\n[{}] songs in queue",
                cold_queue
                    .queue.clone()
                    .drain(..)
                    .chain(cold_queue.radio_queue.clone().drain(..))
                    .chain(
                        cold_queue.radio_next
                        .iter()
                        .filter_map(
                            |(_next, metadata)|
                            metadata
                                .clone()
                                .map(|x| {
                                    let metadata: Metadata = x.into();
                                    metadata.source_url.unwrap_or("Unknown".to_string())
                                })
                        )
                    )
                    .collect::<Vec<String>>()
                    .join("\n"),

                cold_queue.queue.len()
            )
       ).await?;

    return Ok(());
}


#[command]
#[only_in(guilds)]
async fn shuffle(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    tracing::info!(
        "[{}::{}] shuffled playlist in [{}::{:?}]",
        msg.author.id, msg.author.name,
        msg.channel_id, msg.channel_id.name(&ctx).await
    );

    let qctx = ctx.data.write().await
        .get_mut::<LazyQueueKey>().unwrap()
        .get_mut(&guild_id).unwrap().clone();

    {
        use rand::thread_rng;
        use rand::seq::SliceRandom;

        let mut write_lock = qctx.cold_queue.write().await;

        let mut vec = write_lock.queue.iter().cloned().collect::<Vec<_>>();

        vec.shuffle(&mut thread_rng());
        write_lock.queue.clear();
        write_lock.queue.extend(vec);
    }

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.get(guild_id) {
        Some(x) => x,
        None => {
            msg.channel_id
               .say(&ctx.http, "Not in a voice channel to play in")
               .await?;
            return Ok(())
        }
    };

    msg.channel_id
       .say(
            &ctx.http,
            format!("shuffled."),
       )
       .await?;

    let mut call = handler_lock.lock().await;
    let queue = call.queue();
    let _ = queue.skip();

    Ok(())
}

#[command]
async fn q_autoindex(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            msg.channel_id
               .say(&ctx.http, "Must provide a URL to a video or audio")
               .await
               .unwrap();
            return Ok(());
        },
    };

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let qctx: Arc<QueueContext>;

    let call = match manager.get(guild_id) {
        Some(call_lock) => {
            qctx = ctx.data.write().await.get_mut::<LazyQueueKey>().unwrap().get_mut(&guild_id).unwrap().clone();
            call_lock
        },
        None => {
            let tmp = join_routine(ctx, msg).await;

            if let Err(ref e) = tmp {
                msg.channel_id
                   .say(&ctx.http, format!("Failed to join voice channel: {:?}", e))
                   .await
                   .unwrap();
                return Ok(());
            };
            qctx = tmp.unwrap();
            msg.channel_id
                   .say(&ctx.http, format!("Joined: {}", qctx.voice_chan_id.mention()))
                   .await
                   .unwrap();

            let call = manager.get(guild_id).ok_or_else(|| JoinError::NoCall);
            call?
        }
    };

    let response = reqwest::get(url.clone()).await?;
    let html_content = response.text().await?;
    let re = regex::Regex::new(r#"<a href="([^"]+)">"#)?;

    let mut urls = re.captures_iter(&html_content)
      .map(|x| x[1].to_string())
      .filter(|x| x.as_str() != "../")
      .map(|x| format!("{}/{}", url, x))
      .collect::<Vec<_>>();

    let how_many = urls.len();
    let mut urls_drain = urls.drain(..);
    qctx.cold_queue.write().await.queue.extend(urls_drain);
    msg.reply(ctx, format!("added: [{}]", how_many)).await;
    Ok(())
}
