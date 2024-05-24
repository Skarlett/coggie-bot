use serenity::{
    model::{channel::Message, prelude::*}, 
    prelude::*, 
};

use songbird::{
    create_player,
     error::{JoinError, JoinResult},
      events::Event, 
input::Input, 
    tracks::{Track, TrackHandle}, Call, 
   Songbird, TrackEvent
};

use std::{
    time::{Duration, Instant},
    collections::VecDeque,
    sync::Arc,
    path::PathBuf,
};

use std::sync::atomic::AtomicBool;
use parking_lot::{lock_api::GuardNoSend, Mutex};

use tokio::io::AsyncWriteExt;
use serenity::futures::StreamExt;
use core::sync::atomic::Ordering;

use crate::models::*;
use crate::compat::*;

const TS_PRELOAD_OFFSET: Duration = Duration::from_secs(20);
const TS_CROSSFADE_OFFSET: Duration = Duration::from_secs(10);
const TS_ABANDONED_HB: Duration = Duration::from_secs(720);
const HASPLAYED_MAX_LEN: usize = 10;

#[derive(PartialEq, Eq)]
pub enum Players {
    Ytdl,
    Deemix,
    HttpGet,
}

impl Players {
    pub fn from_str(data : &str) -> Option<Self>
    {
        const DEEMIX: [&'static str; 4] = ["deezer.page.link", "deezer.com", "open.spotify", "spotify.link"];
        const YTDL: [&'static str; 4] = ["youtube.com", "youtu.be", "music.youtube.com", "soundcloud.com"];
        const HTTPGET: [&'static str; 3] = [
            "tape.unallocatedspace.luni",
            "tape.cypress.local",
            "vxsesh.cypress.local"
        ];

        if DEEMIX.iter().any(|x|data.contains(x)) { return Some(Self::Deemix) }
        else if YTDL.iter().any(|x|data.contains(x)) {return Some(Self::Ytdl) }
        else if HTTPGET.iter().any(|x|data.contains(x)) {return Some(Self::HttpGet) }
        else { return None }
    }

    pub async fn into_input(&self, uri: &str, guild_id: GuildId) -> Result<(Input, Option<MetadataType>), HandlerError>
    {
        match self {
            Self::Deemix => ph_deemix_player(uri).await,
            Self::Ytdl => ph_ytdl_player(uri).await,
            Self::HttpGet => {
                let mut pathbuf = PathBuf::new();
                let result = ph_httpget_player(uri, guild_id.0, &mut pathbuf).await;
                match result {
                    Ok((input, metadata)) => {
                        // let (_track, track_handle) = create_player(input);
                        // let fp = match metadata {
                        //     Some(MetadataType::Disk(fp)) => fp,
                        //     _ => { return Err(HandlerError::WrongMetadataType) }
                        // };

                        // let _ = track_handle.add_event(Event::Track(TrackEvent::End), RemoveTempFile(fp));

                        // // TODO FIXME ADD METADATA
                        return Ok((input, metadata))
                    }

                    Err(e) => {
                        // cleanup(fp)
                        // TODO FIXME
                        return Err(e)
                    }
                }
            }
        }
    }

    /// turn a uri into a loaded process
    pub async fn create_player(&self, uri: &str, guild_id: GuildId) -> Result<(Track, TrackHandle, Option<MetadataType>), HandlerError>
    {
        let input = self.into_input(uri, guild_id).await;
        match input {
            Ok((input, metadata)) => {
                let (track, track_handle) = create_player(input);
                // (track, track_handle, metadata)
                match (self, metadata.as_ref()) {
                    #[cfg(feature = "http-get")]
                    (Self::HttpGet, Some(MetadataType::Disk(fp))) => {
                        let _ = track_handle.add_event(Event::Track(TrackEvent::End), crate::events::RemoveTempFile(fp.clone()));
                    }
                    (Self::HttpGet, _) => return Err(HandlerError::WrongMetadataType),
                    _ => {}
                
                }
                return Ok((track, track_handle, metadata))        
            }
            Err(e) => return Err(e)
        };
    }

    pub async fn fan_collection(&self, uri: &str) -> Result<VecDeque<String>, HandlerError> {
        let mut buf = VecDeque::new();
        match self {
            Self::HttpGet => {buf.push_back(uri.to_owned()); Ok(1)},
            Self::Deemix => fan_deezer(uri, &mut buf).await,
            Self::Ytdl => fan_ytdl(uri, &mut buf).await 
        }?;

        return Ok(buf)
    }
}

pub async fn play(
    call: &mut Call,
    track: Track,
    handle: &TrackHandle,
    cold_queue: &mut ColdQueue,
    crossfade: bool,
) -> Result<(), HandlerError>
{
    if ! crossfade {
        call.enqueue(track);
        tracing::info!("playing track with builtin-queue");
        return Ok(());
    }
    
    // track.pause();
    call.play(track);
    tracing::info!("playing track with crossfading");
    
    match (cold_queue.crossfade_lhs.take(), cold_queue.crossfade_rhs.take()) {    
        (Some(lhs), Some(rhs)) => { 
            cold_queue.crossfade_lhs = Some(lhs);
            cold_queue.crossfade_rhs = Some(rhs);
            return Err(HandlerError::CrossFadeHandleExhaust);
        }

        (Some(lhs), None) => {
            cold_queue.crossfade_lhs = Some(lhs);
            let _ = handle.make_playable();
            cold_queue.crossfade_rhs = Some(handle.clone());
        }

        (None, None) => {
            let _ = handle.make_playable();
            let _ = handle.play();
            cold_queue.crossfade_lhs = Some(handle.clone());
        }
        (None, Some(rhs)) => {
            cold_queue.crossfade_lhs = Some(rhs);
            
            let _ = handle.make_playable();
            cold_queue.crossfade_rhs = Some(handle.clone());
        }        
    }

    Ok(())
}

#[cfg(feature = "http-get")]
pub fn human_filesize(n: u64) -> String {
    let base: u64 = 1024;
    let suffixes = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    let i = (n as f64).log(base as f64).floor() as u32;
    let power = base.pow(i);
    let size = n as f64 / power as f64;
    return format!("{}{}", size, suffixes[i as usize]);
}

#[cfg(feature = "http-get")]
pub async fn get_file(
    uri: &str,
    vcid: u64,
    fp: &PathBuf,
    // key: [u8; 16]
) -> Result<Input, HandlerError> {

    let client = reqwest::ClientBuilder::new()
        .https_only(false)
        .tls_sni(false)
        .build()?;

    let resp = client.get(uri).send().await?;
    let headers = resp.headers();
    let content_type = headers.get("Content-Type").unwrap();
    // let content_disposition = headers.get("Content-Disposition").unwrap();

    let content_type = content_type.to_str().unwrap();
    match content_type {
        "audio/x-flac" | "audio/mpeg" | "audio/wav" | "audio/x-wav" => {
            // let content_disposition = headers.get("Content-Disposition").unwrap();
            // Content-Disposition: attachment; filename*=UTF-8''Geostigma.mp3
            // let filename = content_disposition.to_str().unwrap().split("filename*=UTF-8''").last().unwrap();
            tracing::info!("writing: {}", fp.display());
            let mut fd = tokio::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(&fp)
                .await?;

            let mut stream = resp.bytes_stream();
            while let Some(item) = stream.next().await {
                let chunk = &item?;
                fd.write_all(chunk).await?;
            }

            fd.flush().await?;
            fd.sync_all().await?;

            tracing::info!("wrote: {} [{}]", fp.display(), human_filesize(fd.metadata().await?.len()));

            let input = songbird::input::ffmpeg(&fp).await.map_err(HandlerError::from);
            input
        }

        content_type => {
            tracing::error!("{}: content type is not supported", uri);
            return Err(HandlerError::UnsupportedMediaType(content_type.to_owned()))
        }
    }
}

async fn add_events(handle: &TrackHandle, qctx_arc: Arc<QueueContext>, crossfading: bool)
{
    if let Some(duration) = handle.metadata().duration {
        if duration < TS_PRELOAD_OFFSET {
            tracing::warn!("No duration provided, preloading disabled");
        }
        tracing::info!("Preload Event Added from Duration");
        
        handle.add_event(
            Event::Delayed(duration - TS_PRELOAD_OFFSET),
            crate::events::PreloadInvoker::new(qctx_arc.clone())
        ).unwrap();

        if crossfading {
            tracing::info!("CrossFade Event Added from Duration"); 
            
            handle.add_event(
                Event::Periodic(duration - TS_CROSSFADE_OFFSET, Some(Duration::from_millis(100))),
                crate::crossfade::CrossFadeInvoker(qctx_arc.clone())
            ).unwrap();
        }
    }

    else { 
        tracing::warn!("No duration provided, preloading disabled");
        if qctx_arc.crossfade.load(Ordering::Relaxed)  {
            tracing::warn!("No duration provided, crossfade disabled");
        }
    }
}

async fn history_completed_track(has_played: &mut VecDeque<TrackRecord>, metadata: MetadataType) {
    if has_played.len() > HASPLAYED_MAX_LEN {
        let _ = has_played.pop_back();
    }
    // --- START
    // This portion of code marks songs as finished or not.
    // Under normal circumstances, this would be placed on the "EndTrack"
    // Event. It also happens that pausing, skipping, and leaving
    // all cause this event to fire.
    // So instead, its placed here to avoid those.
    if let Some(x) = has_played.front_mut() {
        if let EventEnd::UnMarked = x.stop_event {
            x.stop_event = EventEnd::Finished;
            x.end = Instant::now();
        }
    }

    let data = TrackRecord {
        metadata,
        stop_event: EventEnd::UnMarked,
        start: Instant::now(),
        end: Instant::now(),
    };

    has_played.push_front(data);
}

pub async fn next_track_handle(
    cold_queue: &mut ColdQueue,
    qctx: Arc<QueueContext>,
    crossfade: bool
) -> Result<Option<(Track, TrackHandle, Option<MetadataType>)>, HandlerError>
{   
    if let Some((preload, metadata)) = cold_queue.queue_next.take() {
        tracing::info!("Pulling track from user-preload");
        let (track, handle) = create_player(preload.into());
        add_events(&handle, qctx.clone(), crossfade).await;
        Ok(Some((track, handle, metadata)))
    }

    else if let Ok(Some((track, handle, metadata))) = invoke_cold_queue(cold_queue, qctx.clone()).await {
        tracing::info!("Pulling track from user-queue");
        add_events(&handle, qctx.clone(), crossfade).await;      

        Ok(Some((track, handle, metadata)))
    }

    else if cold_queue.use_radio {
        if let Some((radio_preload, metadata)) = cold_queue.radio_next.take() {
            tracing::info!("Pulling track from radio");
            let (track, handle) = create_player(radio_preload.into());
            add_events(&handle, qctx.clone(), crossfade).await;      
            Ok(Some((track, handle, metadata)))
        }
        else { Ok(None) }
    }
    else { Ok(None) }
}

pub async fn invoke_cold_queue(
    cold_queue: &mut ColdQueue,
    qctx_arc: Arc<QueueContext>
) -> Result<Option<(Track, TrackHandle, Option<MetadataType>)>, HandlerError> {
    let mut tries = 4;

    while let Some(uri) = cold_queue.queue.pop_front() {
        tracing::info!("Now playing: {}", uri);
        let player = Players::from_str(&uri)
            .ok_or_else(|| HandlerError::NotImplemented)?;

        // turn realization to live
        match player.create_player(&uri, qctx_arc.guild_id).await
        {
            Ok((track, handle, metadata)) =>
                return Ok(Some((track, handle, metadata))),

            Err(e) => {
                tracing::error!("Failed to play next track: {}", e);
                let response = match e {
                    HandlerError::NotImplemented 
                        => "Not implemented/enabled".to_string(),
                    
                    HandlerError::NoCall 
                        => "No call found".to_string(),
                    
                    HandlerError::IOError(e) 
                        => format!("IO Error: {}", e.kind()),

                    #[cfg(feature = "http-get")]
                    HandlerError::UnsupportedMediaType(content_type)
                        => format!("Content type is not supported [{}]", content_type),

                    #[cfg(feature = "http-get")]
                    HandlerError::Reqwest(err)
                        => format!("Reqwest error: {}", err),


                    #[cfg(feature = "deemix")]
                    HandlerError::DeemixError(crate::deemix::DeemixError::BadJson(text))
                        => {
                            qctx_arc.invited_from.send_files(
                                &qctx_arc.http,
                                vec![ (text.as_bytes(), "error.txt") ],
                                |m| m
                            ).await?;
                            "Json Error".to_string()
                        }
                    
                    e => format!("Discord error: {}", e)
                };

                if tries == 0 {
                    let _ = qctx_arc.invited_from
                        .say(&qctx_arc.http, format!("Halting. Last try: {}", &uri))
                        .await;
                    break
                }

                let _ = qctx_arc.invited_from
                    .say(&qctx_arc.http, format!("Couldn't play track {}\n{}", &uri, &response))
                    .await;

                tries -= 1;
            }
        }
    }    
    Ok(None)
}

pub async fn leave_routine (
    data: Arc<RwLock<TypeMap>>,
    guild_id: GuildId,
    manager: Arc<Songbird>
) -> JoinResult<()>
{   
    let handler = manager.get(guild_id).unwrap();

    {
        let mut call = handler.lock().await;
        call.remove_all_global_events();
        call.stop();
    }
    
    manager.remove(guild_id).await?;

    {
        let mut glob = data.write().await; 
        let queue = glob.get_mut::<LazyQueueKey>()
            .expect("Expected LazyQueueKey in TypeMap");
        queue.remove(&guild_id);
    }

    Ok(())
}

pub async fn join_routine(ctx: &Context, msg: &Message) -> Result<Arc<QueueContext>, JoinError> {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => {
            tracing::info!(
                "[{}::{}] requested coggie in vc [{}::{:?}]",
                msg.author.id, msg.author.name, msg.channel_id, msg.channel_id.name(&ctx).await
            );
            channel
        },
        None => {
            msg.reply(&ctx.http, "Not in a voice channel").await.unwrap();
            return Err(JoinError::NoCall);
        },
    };

    let chan: Channel = connect_to.to_channel(&ctx.http).await.unwrap();

    let gchan = match chan {
        Channel::Guild(ref gchan) => gchan,
        _ => {
            msg.reply(
              &ctx.http,
              "Not supported voice channel"
            ).await
             .unwrap();

            return Err(JoinError::NoCall);
        }
    };

    match gchan.bitrate
    {
       Some(x) if x > 90_000 => {}
       None => {
           tracing::info!(
               "[{}::{:?}] coggie detected low quality vc",
               msg.channel_id, msg.channel_id.name(&ctx).await
           );
           let _ = msg.reply(
               &ctx.http,
               r#"**Couldn't detect bitrate.** For the best experience,
                  check that the voice room is using 128kbps."#
           ).await;
       }

       Some(x) => {
            tracing::info!(
                "[{}::{:?}] coggie detected low quality vc",
                msg.channel_id, msg.channel_id.name(&ctx).await
            );

            #[cfg(feature = "deemix")]
            let _ = msg.reply(
                &ctx,
                format!(
                    r#"**Low quality voice room** detected.

                    For the best experience, use 128kbps, & spotify links 
                    [Currently: {}kbps]"#,
                    (x / 1000)
                )
            ).await;
        }
    }
    
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let (_handle_lock, success) = manager.join(guild_id, connect_to).await;

    if let Err(e) = success {
        return Err(e);
    }
    
    let call_lock = manager.get(guild_id).unwrap(); 
    let mut call = call_lock.lock().await;

    let queuectx =
        if let Channel::Guild(voice_chan_id) = chan {
            QueueContext {
                guild_id,
                voice_chan_id,
                crossfade: AtomicBool::new(false),
                invited_from: msg.channel_id,
                cache: ctx.cache.clone(),
                data: ctx.data.clone(),
                manager: manager.clone(),
                http: ctx.http.clone(),
                cold_queue: Arc::new(RwLock::new(ColdQueue {
                    queue: VecDeque::new(),
                    has_played: VecDeque::new(),
                    use_radio: false,
                    queue_next: None, //TODO: implement me
                    radio_next: None,
                    radio_queue: VecDeque::new(),
                    crossfade_lhs: None,
                    crossfade_rhs: None,
                })),
                crossfade_step: Mutex::new(1),
            }
        } else {
            tracing::error!("Expected voice channel (GuildChannel), got {:?}", chan);
            return Err(JoinError::NoCall);
        };

    
    let queuectx = Arc::new(queuectx);
    
    {
        let mut glob = ctx.data.write().await; 
        let queue = glob.get_mut::<LazyQueueKey>()
            .expect("Expected LazyQueueKey in TypeMap");
        queue.insert(guild_id, queuectx.clone());
    }

    let _ = call.deafen(true).await;
    
    call.add_global_event(
        Event::Track(TrackEvent::End),
        crate::radio::RadioInvoker::new(queuectx.clone())
    );
    
    call.add_global_event(
        Event::Track(TrackEvent::Play),
        crate::events::StartLog,
    );
     
    call.add_global_event(
        Event::Track(TrackEvent::End),
        crate::events::EndLog,
    );

    call.add_global_event(
        Event::Periodic(TS_ABANDONED_HB, None),
        crate::events::AbandonedChannel(queuectx.clone())
    );

    Ok(queuectx)
}
