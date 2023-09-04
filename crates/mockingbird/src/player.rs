use serenity::{
    async_trait,
    model::channel::Message,
    framework::standard::{
        macros::{command, group},
        CommandResult, Args,
    }, 
    client::Cache,
    http::Http,
    prelude::*,
    model::prelude::*
};

use songbird::{
    error::{JoinResult, JoinError},
    events::{Event, EventContext, TrackEvent},
    EventHandler as VoiceEventHandler,
    Songbird,
    Call, 
    create_player,
    input::{Input, error::Error as SongbirdError}
};

use std::{
    process::Stdio,
    time::Duration, collections::VecDeque,
    sync::Arc,
    collections::HashMap,
};

use tokio::{
    io::AsyncBufReadExt,
    process::Command, time::Instant
};

#[group]
#[commands(njoin, nleave, nqueue, now_playing)]
struct BetterPlayer;


// const MKBIRD_QUEUE_PRELOAD_SECS: &'static str = env!("MKBIRD_QUEUE_PRELOAD_SECS", "20");
// const MKBIRD_QUEUE_MAX_SONGS: &'static str = env!("MKBIRD_QUEUE_MAX_SONGS", "0");
// const MKBIRD_QUEUE_MAX_ENQUEUE: &'static str = env!("MKBIRD_QUEUE_MAX_ENQUEUE", "0");
// const MKBIRD_QUEUE_MAX_DURATION: &'static str = env!("MKBIRD_QUEUE_MAX_DURATION", "0");
// const MKBIRD_QUEUE_SCHEDULER_MS: &'static str = env!("MKBIRD_QUEUE_MAX_DURATION", "350");

const TS_ZERO: Duration = Duration::from_secs(0);

#[allow(unused_variables)]
#[derive(Debug)]
enum HandlerError {
    Songbird(SongbirdError),
    IOError(std::io::Error),
    Serenity(serenity::Error),
    NotImplemented,
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
        }
    }
}

impl std::error::Error for HandlerError {}

/*
 * Some ugly place holders for
 * feature generated code.
*/
#[cfg(feature="deemix")]
async fn fan_deezer(uri: &str, buf: &mut Vec<String>) -> Result<(), HandlerError> {
    let mut json_buf = Vec::new();
    _urls("deemix-metadata", &[uri], &mut json_buf).await?;
    buf.extend(
        json_buf.iter()
            .map(|x| x["link"].as_str().unwrap().to_owned())
    );
    Ok(())
}

#[cfg(feature="ytdl")]
async fn fan_ytdl(uri: &str, buf: &mut Vec<String>) -> Result<(), HandlerError> {
    let mut json_buf = Vec::new();
    _urls("yt-dlp", &["--flat-playlist", "-j", uri], &mut json_buf).await?;
    
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
        const DEEMIX: [&'static str; 3] = ["deezer.page.link", "deezer.com", "open.spotify"];
        const YTDL: [&'static str; 4] = ["youtube.com", "youtu.be", "music.youtube.com", "soundcloud.com"];

        if DEEMIX.iter().any(|x|data.contains(x)) { return Some(Self::Deemix) }
        else if YTDL.iter().any(|x|data.contains(x)) {return Some(Self::Ytdl) }
        else { return None }
    }

    async fn play(&self, handler: &mut Call, uri: &str) -> Result<(), HandlerError>
    {
        let input = match self {
            Self::Deemix => ph_deemix_player(uri).await,
            Self::Ytdl => ph_ytdl_player(uri).await
        }?;

        let (track, _track_handle) = create_player(input);
        handler.enqueue(track);
        Ok(())
    }

    async fn fan_collection(&self, uri: &str) -> Result<Vec<String>, HandlerError> {
        let mut buf = Vec::new();
        match self {
            Self::Deemix => fan_deezer(uri, &mut buf).await,
            Self::Ytdl => fan_ytdl(uri, &mut buf).await 
        }?;
        return Ok(buf)
    }
}

type LazyQueue = HashMap<GuildId, Arc<RwLock<VecDeque<String>>>>;
pub struct LazyQueueKey;
impl TypeMapKey for LazyQueueKey {
    type Value = LazyQueue;
}


struct QueueContext {
    guild_id: GuildId,
    invited_from: ChannelId,
    voice_chan_id: GuildChannel,
    http: Arc<Http>,
    cache: Arc<Cache>,
    data: Arc<RwLock<TypeMap>>,
    manager: Arc<Songbird>,
    cold_queue: Arc<RwLock<VecDeque<String>>>,
    lock: Mutex<()>
}

struct AbandonedChannel(Arc<QueueContext>);

#[async_trait]
impl VoiceEventHandler for AbandonedChannel {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
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

struct TrackEndNotifier(Arc<QueueContext>);
#[async_trait]
impl VoiceEventHandler for TrackEndNotifier {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(track_list) = ctx {
            let cold_queue_len = self.0.cold_queue.read().await.len();
            let hot_queue_len = track_list.len();
            let total = cold_queue_len + hot_queue_len;

            self.0
                .invited_from
                .say(&self.0.http, &format!("Tracks ended: ({} left).", total))
                .await
                .unwrap();
            
            tracing::info!("Hot queue: {}", track_list.len());
            tracing::info!("Cold queue: {}", self.0.cold_queue.read().await.len());
        }
        None
    }
}

struct GlobalPreloader(Arc<QueueContext>);

impl GlobalPreloader {
    async fn next_song(uri: &str, call: &mut Call) -> Result<(), HandlerError> {
        let player = Players::from_str(&uri)
            .ok_or_else(|| HandlerError::NotImplemented)?;
        
        player.play(call, &uri).await?;
        Ok(())
    }

    async fn handler_error(&self, why: HandlerError, uri: &str) {
        self.0.invited_from
            .say(&self.0.http, format!("Failed to play song. [{}] ({})", uri, why))
            .await
            .unwrap();
    }
}

#[async_trait]
impl VoiceEventHandler for GlobalPreloader {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        const TS_PRELOAD_OFFSET: Duration = Duration::from_secs(20);
        let before_lock = std::time::Instant::now();
        let _ = self.0.lock.lock().await;

        if before_lock.elapsed() > Duration::from_millis(1000) {
            tracing::info!("Skipping preloader tick, lock took too long");
            return None;
        }

        if self.0.cold_queue.read().await.is_empty() {
            return None;
        }

        match self.0.manager.get(self.0.guild_id) {
            Some(call_lock) => {
                let mut call = call_lock.lock().await;
                let hotqueue = call.queue();
                tracing::info!("Hot queue: {}/{}", hotqueue.len(), self.0.cold_queue.read().await.len());
                match hotqueue.len() {
                    1 => {
                            let current = hotqueue.current().unwrap();
                            let current_meta = current.metadata().clone();
                            let state = current.get_info().await.unwrap();

                            tracing::info!("Current: {:?}", current.metadata().duration);
                            tracing::info!("State::position: {:?}", state.position);
                            tracing::info!("State::playtime: {:?}", state.play_time);

                            let total_duration = current_meta.duration.unwrap();
                            
                            if state.position >= total_duration - TS_PRELOAD_OFFSET {
                                let next_song = self.0.cold_queue.write().await.pop_front().unwrap();
                                let player = Players::from_str(&next_song).unwrap();
                                player.play(&mut call, &next_song).await.unwrap();

                                if let Err(why) = Self::next_song(&next_song, &mut call).await {
                                    self.handler_error(why, &next_song).await;
                                }
                            }
                        }

                    0 => { 
                        let next_song = self.0.cold_queue.write().await.pop_front().unwrap();
                        if let Err(why) = Self::next_song(&next_song, &mut call).await {
                            self.handler_error(why, &next_song).await;
                        } 
                    },
                    _ => {}
                }
                return None
            },
            None => unreachable!()
        }
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

async fn join_routine(ctx: &Context, msg: &Message) -> Result<ChannelId, JoinError> {
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
            //TODO: fix
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



    // cold queue 
    let rc = Arc::new(RwLock::new(VecDeque::new()));
    {
        let mut glob = ctx.data.write().await; 
        let queue = glob.get_mut::<LazyQueueKey>().expect("Expected LazyQueueKey in TypeMap");
        queue.insert(guild_id, rc.clone());
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
                http: ctx.http.clone(),
                cache: ctx.cache.clone(),
                data: ctx.data.clone(),
                manager: manager.clone(),
                cold_queue: rc.clone(),
                lock: Mutex::new(())
            }
        } else {
            tracing::error!("Expected voice channel (GuildChannel), got {:?}", chan);
            return Err(JoinError::NoCall);
        };

    let queuectx = Arc::new(queuectx);

    call.add_global_event(
        Event::Track(TrackEvent::End),
        TrackEndNotifier(queuectx.clone())
    );

    call.add_global_event(
        Event::Periodic(Duration::from_secs(5), Some(Duration::from_secs(1))),
        GlobalPreloader(queuectx.clone())
    );


    call.add_global_event(
        Event::Periodic(Duration::from_secs(720), None),
        AbandonedChannel(queuectx.clone())
    );

    Ok(connect_to)
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

    msg.channel_id
       .say(&ctx.http, format!("Joined {}", connect_to.unwrap().mention()))
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
       .say(&ctx.http, format!("Joined {}", connect_to.unwrap().mention()))
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

    match manager.get(guild_id) {
        Some(_call_lock) => { },
        None => {
            let connected_to = join_routine(ctx, msg).await;            
            
            if let Err(ref e) = connected_to {
                msg.channel_id
                   .say(&ctx.http, format!("Failed to join voice channel: {:?}", e))
                   .await
                   .unwrap();        
                return Ok(());
            };

            msg.channel_id
                   .say(&ctx.http, format!("Joined voice channel: {:?}", connected_to.unwrap().mention()))
                   .await
                   .unwrap();

            manager.get(guild_id).unwrap();
        }
    };

    match Players::from_str(&url)
        .ok_or_else(|| String::from("Failed to select extractor for URL"))
    {
        Ok(player) => {
            let mut uris = player.fan_collection(url.as_str()).await?;
            let added = uris.len();

            {
                let mut glob = ctx.data.write().await; 
                let queue = glob.get_mut::<LazyQueueKey>().expect("Expected LazyQueueKey in TypeMap");
                queue.get_mut(&guild_id).unwrap().write().await.extend(uris.drain(..));
            }
            
            msg.channel_id
               .say(&ctx.http, format!("Added {} Song(s)", added))
               .await?;
            // let mut call = call.lock().await;
            // player.play(&ctx.http, &mut call, &url).await?;
        },

        Err(_) => {
            msg.channel_id
               .say(&ctx.http, format!("Failed to select extractor for URL: {}", url))
               .await?;
        }
    }

    Ok(())
}


