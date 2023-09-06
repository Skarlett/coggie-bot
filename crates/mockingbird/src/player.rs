use serenity::{
    async_trait,
    model::channel::Message,
    framework::standard::{
        macros::{command, group},
        CommandResult, Args,
    }, 
    client::Cache,
    prelude::*,
    model::prelude::*, http::Http
};

use songbird::{
    error::{JoinResult, JoinError},
    events::{Event, EventContext},
    EventHandler as VoiceEventHandler,
    Songbird,
    Call, 
    create_player,
    input::{Input, error::Error as SongbirdError},
    tracks::{TrackHandle, Track},
    TrackEvent
};

use std::{
    process::Stdio,
    time::Duration, collections::VecDeque,
    sync::Arc,
    collections::HashMap,
};

use tokio::{
    io::AsyncBufReadExt,
    process::Command,
};

const TS_PRELOAD_OFFSET: Duration = Duration::from_secs(20);
const TS_ABANDONED_HB: Duration = Duration::from_secs(720);

#[group]
#[commands(njoin, nleave, nqueue, now_playing, nskip)]
struct BetterPlayer;

async fn next_track(call: &mut Call, uri: &str) -> Result<TrackHandle, HandlerError> {
    let player = Players::from_str(&uri)
        .ok_or_else(|| HandlerError::NotImplemented)?;
    
    player.play(call, &uri).await.map_err(HandlerError::from)
}

#[allow(unused_variables)]
#[derive(Debug)]
enum HandlerError {
    Songbird(SongbirdError),
    IOError(std::io::Error),
    Serenity(serenity::Error),
    NotImplemented,
    NoCall
}

impl From<serenity::Error> for HandlerError {
    fn from(err: serenity::Error) -> Self {
        HandlerError::Serenity(err)
    }
}

impl From<SongbirdError> for HandlerError {
    fn from(err: SongbirdError) -> Self {
        HandlerError::Songbird(err)
    }
}

impl From<std::io::Error> for HandlerError {
    fn from(err: std::io::Error) -> Self {
        HandlerError::IOError(err)
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Songbird(err) => write!(f, "Songbird error: {}", err),
            Self::NotImplemented => write!(f, "This feature is not implemented."),
            Self::IOError(err) => write!(f, "IO error: (most likely deemix-metadata failed) {}", err),
            Self::Serenity(err) => write!(f, "Serenity error: {}", err),
            Self::NoCall => write!(f, "Not in a voice channel to play in")
        }
    }
}
impl std::error::Error for HandlerError {}

/*
 * Some ugly place holders for
 * feature generated code.
*/
#[cfg(feature="deemix")]
async fn fan_deezer(uri: &str, buf: &mut VecDeque<String>) -> Result<(), HandlerError> {
    let mut json_buf = Vec::new();
    _urls("deemix-metadata", &[uri], &mut json_buf).await?;
    
    // FIXME: Don't use unwrap, check for key "error"
    buf.extend(
        json_buf.iter()
            .map(|x| 
                x["link"].as_str().unwrap().to_owned())
    );
    Ok(())
}

#[cfg(feature="ytdl")]
async fn fan_ytdl(uri: &str, buf: &mut VecDeque<String>) -> Result<(), HandlerError> {
    let mut json_buf = Vec::new();
    _urls("yt-dlp", &["--flat-playlist", "-j", uri], &mut json_buf).await?;
    
    // FIXME: Don't use unwrap, check for key "error"
    buf.extend(
        json_buf.iter()
           .map(|x| x["url"].as_str().unwrap().to_owned())
    );
    Ok(())
}

#[cfg(not(feature="deemix"))]
async fn fan_deezer(uri: &str, buf: &mut Vec<String>) -> Result<(), HandlerError>  {
    return Err(HandlerError::NotImplemented)
}

#[cfg(not(feature="ytdl"))]
async fn fan_ytdl(uri: &str, buf: &mut Vec<String>) -> Result<(), HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(feature = "deemix")]
async fn ph_deemix_player(uri: &str) -> Result<Input, HandlerError> {
    crate::deemix::deemix(uri).await.map_err(HandlerError::from)
}

#[cfg(feature = "ytdl")]
async fn ph_ytdl_player(uri: &str) -> Result<Input, HandlerError> {
    return songbird::ytdl(uri).await.map_err(HandlerError::from)
}

#[cfg(not(feature = "deemix"))]
async fn ph_deemix_player(uri: &str) -> Result<Input, HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(not(feature = "ytdl"))]
async fn ph_ytdl_player(uri: &str) -> Result<Input, HandlerError> {
    return Err(HandlerError::NotImplemented)
}

async fn _urls(cmd: &str, args: &[&str], buf: &mut Vec<serde_json::Value>) -> std::io::Result<()> {
    let child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.wait_with_output().await.unwrap();
    let mut lines = stdout.stdout.lines();
   
    while let Some(line) = lines.next_line().await? {
        let json = serde_json::from_str(&line).unwrap();
        buf.push(json);
    } 
    Ok(())
}

enum Players {
    Ytdl,
    Deemix,
}

impl Players {
    fn from_str(data : &str) -> Option<Self>
    {
        const DEEMIX: [&'static str; 4] = ["deezer.page.link", "deezer.com", "open.spotify", "spotify.link"];
        const YTDL: [&'static str; 4] = ["youtube.com", "youtu.be", "music.youtube.com", "soundcloud.com"];

        if DEEMIX.iter().any(|x|data.contains(x)) { return Some(Self::Deemix) }
        else if YTDL.iter().any(|x|data.contains(x)) {return Some(Self::Ytdl) }
        else { return None }
    }

    async fn play(&self, handler: &mut Call, uri: &str) -> Result<TrackHandle, HandlerError>
    {
        let input = match self {
            Self::Deemix => ph_deemix_player(uri).await,
            Self::Ytdl => ph_ytdl_player(uri).await
        }?;

        let (track, track_handle) = create_player(input);
        handler.enqueue(track);
        Ok(track_handle)
    }

    async fn fan_collection(&self, uri: &str) -> Result<VecDeque<String>, HandlerError> {
        let mut buf = VecDeque::new();
        match self {
            Self::Deemix => fan_deezer(uri, &mut buf).await,
            Self::Ytdl => fan_ytdl(uri, &mut buf).await 
        }?;
        return Ok(buf)
    }
}

type LazyQueue = HashMap<GuildId, Arc<QueueContext>>;
pub struct LazyQueueKey;
impl TypeMapKey for LazyQueueKey {
    type Value = LazyQueue;
}

pub struct QueueContext {
    guild_id: GuildId,
    invited_from: ChannelId,
    voice_chan_id: GuildChannel,
    cache: Arc<Cache>,
    data: Arc<RwLock<TypeMap>>,
    http: Arc<Http>,
    manager: Arc<Songbird>,
    cold_queue: Arc<RwLock<VecDeque<String>>>,
}

struct AbandonedChannel(Arc<QueueContext>);
#[async_trait]
impl VoiceEventHandler for AbandonedChannel {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let members = self.0.voice_chan_id.members(&self.0.cache).await.unwrap();
        if members.iter().filter(|x| !x.user.bot).count() > 0 {
            return None;
        }

        leave_routine(
            self.0.data.clone(),
            self.0.guild_id.clone(),
            self.0.manager.clone()
        ).await.unwrap();

        Some(Event::Cancel)
    }
}

async fn play_routine(qctx: Arc<QueueContext>) -> Result<(), HandlerError> {
    let mut tries = 4;
    let handler = qctx.manager.get(qctx.guild_id).ok_or_else(|| HandlerError::NoCall)?;    
    let mut call = handler.lock().await;

    while let Some(uri) = qctx.cold_queue.write().await.pop_front() {
        match next_track(&mut call, &uri).await {
            Ok(track) => {
                track.add_event(
                    Event::Delayed(track.metadata().duration.unwrap() - TS_PRELOAD_OFFSET),
                    PreemptLoader(qctx.clone())
                ).unwrap();
                break
            },
            Err(e) => {
                if tries == 0 {
                    tracing::error!("Failed to play next track: {}", e);
                    break
                }

                let _ = qctx.invited_from
                    .say(&qctx.http, format!("Couldn't play track {}", &uri))
                    .await;
                tracing::error!("Failed to play next track: {}", e);
                tries -= 1;
            }
        }
    }
    Ok(())
}

struct TrackEndLoader(Arc<QueueContext>);
#[async_trait]
impl VoiceEventHandler for TrackEndLoader {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let mut run = false;

        if let Some(call) = self.0.manager.get(self.0.guild_id) {
          let call = call.lock().await;
          run = call.queue().current().is_none();
        }

        if run {
            let _ = play_routine(self.0.clone()).await;
        }
        None
    }
}

struct PreemptLoader(Arc<QueueContext>);
#[async_trait]
impl VoiceEventHandler for PreemptLoader {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {        
        let _ = play_routine(self.0.clone()).await;
        None
    }
}

async fn leave_routine (
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

async fn join_routine(ctx: &Context, msg: &Message) -> Result<Arc<QueueContext>, JoinError> {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            msg.reply(&ctx.http, "Not in a voice channel").await.unwrap();
            return Err(JoinError::NoCall);
        },
    };

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

    let chan: Channel  = connect_to.to_channel(&ctx.http).await.unwrap();
    
    let queuectx =
        if let Channel::Guild(voice_chan_id) = chan {
            QueueContext {
                guild_id,
                voice_chan_id,
                invited_from: msg.channel_id,
                cache: ctx.cache.clone(),
                data: ctx.data.clone(),
                manager: manager.clone(),
                http: ctx.http.clone(),
                cold_queue: Arc::new(RwLock::new(VecDeque::new())),
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
        TrackEndLoader(queuectx.clone())
    );

    call.add_global_event(
        Event::Periodic(TS_ABANDONED_HB, None),
        AbandonedChannel(queuectx.clone())
    );

    Ok(queuectx)
}

#[command]
#[aliases("np", "playing", "now-playing", "playing-now", "nowplaying")]
#[only_in(guilds)]
async fn now_playing(ctx: &Context, msg: &Message) -> CommandResult {
    let connect_to = join_routine(&ctx, msg).await;
    if let Err(ref e) = connect_to {
        msg.channel_id
           .say(&ctx.http, format!("Failed to join voice channel: {:?}", e))
           .await?;        
    }

    let connect_to = connect_to.unwrap();

    msg.channel_id
       .say(&ctx.http, format!("{}: <link>", connect_to.voice_chan_id.mention()))
       .await?;

    Ok(())
}


#[command]
#[only_in(guilds)]
async fn njoin(ctx: &Context, msg: &Message) -> CommandResult {
    let connect_to = join_routine(&ctx, msg).await;
    
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
async fn nleave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("songbird voice client placed in at initialisation.")
        .clone();

    let handler = manager.get(guild_id);
    
    if !handler.is_some() {
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
// #[aliases("play")]
#[only_in(guilds)]
async fn nqueue(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
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
                   .say(&ctx.http, format!("Joined voice channel: {:?}", qctx.voice_chan_id.mention()))
                   .await
                   .unwrap();

            let call = manager.get(guild_id).ok_or_else(|| JoinError::NoCall);
            call?
        }
    };

    match Players::from_str(&url)
        .ok_or_else(|| String::from("Failed to select extractor for URL"))
    {
        Ok(player) => {
            let mut uris = player.fan_collection(url.as_str()).await?;
            let added = uris.len();
            let call = call.lock().await;
            
            let maybe_hot = call.queue().len() > 0;            

            if !maybe_hot {
                play_routine(qctx.clone()).await?;
            }
            
            qctx.cold_queue.write().await.extend(uris.drain(..));    
            msg.channel_id            
               .say(&ctx.http, format!("Added {} Song(s)", added))
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
async fn nskip(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    
    let qctx = ctx.data.write().await
        .get_mut::<LazyQueueKey>().unwrap()
        .get_mut(&guild_id).unwrap().clone();
     
    let skipn = args.remains()
        .unwrap_or("1")
        .parse::<isize>()
        .unwrap_or(1);

    if skipn < 1 {
        msg.channel_id
           .say(&ctx.http, "Must skip at least 1 song")
           .await?;
        return Ok(())
    }

    else if skipn > qctx.cold_queue.read().await.len() as isize + 1 {
        qctx.cold_queue.write().await.clear();
        return Ok(())
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

    let mut call = handler_lock.lock().await;

    while let Some(uri) = qctx.cold_queue.write().await.pop_front() { 
        match next_track(&mut call, &uri).await {
            Ok(handle) => {
                handle.add_event(
                    Event::Delayed(handle.metadata().duration.unwrap() - TS_PRELOAD_OFFSET),
                    PreemptLoader(qctx.clone())
                )?;
                break;
            },
            Err(e) => {
                msg.channel_id
                   .say(&ctx.http, format!("Couldn't play track {}", &uri))
                   .await?;
                tracing::error!("Failed to play next track: {}", e);
            }
        }            
    }
    let cold_queue_len = qctx.cold_queue.read().await.len();

    msg.channel_id
       .say(
            &ctx.http,
            format!("Song skipped [{}]: {} in queue.", skipn, cold_queue_len),
        )
        .await?;

    let queue = call.queue();
    let _ = queue.skip();

    Ok(())
}
