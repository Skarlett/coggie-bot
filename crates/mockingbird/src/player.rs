use serenity::{
    async_trait,
    model::channel::Message,
    framework::standard::{
        macros::{command, group},
        CommandResult, Args,
    }, 
    client::Cache,
    prelude::*,
    model::prelude::*, http::Http, json
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

use cutils::{availbytes, bigpipe, max_pipe_size};

const TS_PRELOAD_OFFSET: Duration = Duration::from_secs(20);
const TS_ABANDONED_HB: Duration = Duration::from_secs(720);
// const MAX_TRACK_LENGTH: Duration = Duration::from_secs(360*6); // 30 minutes
// const MAX_ENQUEUED: u16 = 300;

#[group]
#[commands(join, leave, queue, now_playing, skip)]
struct BetterPlayer;

async fn next_track(call: &mut Call, uri: &str) -> Result<TrackHandle, HandlerError> {
    let player = Players::from_str(&uri)
        .ok_or_else(|| HandlerError::NotImplemented)?;
    
    player.play(call, &uri).await
}

#[allow(unused_variables)]
#[derive(Debug)]
enum HandlerError {
    Songbird(SongbirdError),
    IOError(std::io::Error),
    Serenity(serenity::Error),
    
    #[cfg(feature = "deemix")]
    DeemixError(crate::deemix::DeemixError),
    
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

#[cfg(feature = "deemix")]
impl From<crate::deemix::DeemixError> for HandlerError {
    fn from(err: crate::deemix::DeemixError) -> Self {
        HandlerError::DeemixError(err)
    }
}


impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Songbird(err) => write!(f, "Songbird error: {}", err),
            Self::NotImplemented => write!(f, "This feature is not implemented."),
            
            Self::IOError(err)
                => write!(f, "IO error: (most likely deemix-metadata failed) {}", err),
            
            Self::Serenity(err)
                => write!(f, "Serenity error: {}", err),
            
            Self::NoCall
                => write!(f, "Not in a voice channel to play in"),
            
            #[cfg(feature = "deemix")]
            Self::DeemixError(crate::deemix::DeemixError::BadJson(err))
                => write!(f, "Deemix error: {}", err),

            _ => write!(f, "Unknown error")
        }
    }
}
impl std::error::Error for HandlerError {}

fn process_fan_output(buf: &mut VecDeque<String>, json_buf: Vec<serde_json::Value>, err_cnt: &mut usize, key: &str){
    for x in json_buf {
        if let Some(jmap) = x.as_object() {
            if !jmap.contains_key(key) {
                tracing::error!("{} not found in json", key);
                *err_cnt += 1;
                continue
            }
        
            buf.push_back(jmap[key].as_str().unwrap().to_owned());
        }
        else {

            tracing::error!("{} not found in json", key);
            *err_cnt += 1;
            continue
        }
    }
    tracing::info!("{} tracks found", buf.len());
}

/*
 * Some ugly place holders for
 * feature generated code.
*/
#[cfg(feature="deemix")]
async fn fan_deezer(uri: &str, buf: &mut VecDeque<String>) -> Result<usize, HandlerError> {
    let mut json_buf = Vec::new();
    let mut err_cnt = 0;
    _urls("deemix-metadata", &[uri], &mut json_buf).await?;

    process_fan_output(buf, json_buf, &mut err_cnt, "link");
    Ok(err_cnt)
}

#[cfg(feature="ytdl")]
async fn fan_ytdl(uri: &str, buf: &mut VecDeque<String>) -> Result<usize, HandlerError> {
    let mut json_buf = Vec::new();
    let mut err_cnt = 0;
    _urls("yt-dlp", &["--flat-playlist", "-j", uri], &mut json_buf).await?;
    
    process_fan_output(buf, json_buf, &mut err_cnt, "url");
    Ok(err_cnt)
}

#[cfg(not(feature="deemix"))]
async fn fan_deezer(uri: &str, buf: &mut VecDeque<String>) -> Result<(), HandlerError>  {
    return Err(HandlerError::NotImplemented)
}

#[cfg(not(feature="ytdl"))]
async fn fan_ytdl(uri: &str, buf: &mut VecDeque<String>) -> Result<usize, HandlerError> {
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

#[derive(PartialEq, Eq)]
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

        tracing::info!(
            "[{}::()] coggie leaving abandoned vc",
            channel.id, channel.name
        );

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
    let handler = qctx.manager.get(qctx.guild_id)
        .ok_or_else(|| HandlerError::NoCall)?;
    
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
                tracing::error!("Failed to play next track: {}", e);
                
                let response = match e {
                    HandlerError::NotImplemented 
                        => "Not implemented/enabled".to_string(),
                    
                    HandlerError::NoCall 
                        => "No call found".to_string(),
                    
                    HandlerError::IOError(e) 
                        => format!("IO Error: {}", e.kind()),
                    
                    #[cfg(feature = "deemix")]
                    HandlerError::DeemixError(crate::deemix::DeemixError::BadJson(text))
                        => {
                            qctx.invited_from.send_files(
                                &qctx.http,
                                vec![
                                    (text.as_bytes(), "error.txt")
                                ],
                                |m| m
                            ).await?;
                            "Json Error".to_string()
                        }
                    
                    e => format!("Discord error: {}", e)
                };

                if tries == 0 {
                    let _ = qctx.invited_from
                        .say(&qctx.http, format!("Halting. Last try: {}", &uri))
                        .await;
                    break
                }

                let _ = qctx.invited_from
                    .say(&qctx.http, format!("Couldn't play track {}\n{}", &uri, &response))
                    .await;

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

    tracing::info!(
        "[{}::()] coggie left vc",
        channel.id, channel.name
    );

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
        Some(channel) => {
            tracing::info!(
                "[{}::()] requested coggie in vc [{}::{}]",
                msg.author.id, msg.author.name, channel.id, channel.name
            );
            channel
        },
        None => {
            msg.reply(&ctx.http, "Not in a voice channel").await.unwrap();
            return Err(JoinError::NoCall);
        },
    };

    let chan: Channel  = connect_to.to_channel(&ctx.http).await.unwrap();

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

    match gchan.bitrate {
       Some(x) if x > 90_000 => {}
       None => {
           tracing::info!(
               "[{}::()] coggie detected low quality vc",
               channel.id, channel.name
           );
           let _ = msg.reply(
               &ctx.http,
               r#"**Couldn't detect bitrate.** For the best experience,
                  check that the voice room is using 128kbps."#
           ).await;
       }

       Some(x) => {
            tracing::info!(
                "[{}::()] coggie detected low quality vc",
                channel.id, channel.name
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
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    tracing::info!(
        "[{}::{}] asked what track is playing in [{}::{}]",
        msg.author.id, msg.author.name,
        msg.channel.id, msg.channel.name
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
#[aliases("play")]
#[only_in(guilds)]
async fn queue(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {

    tracing::info!(
        "[{}::{}] queued track in [{}::{}]",
        msg.author.id, msg.author.name,
        msg.channel.id, msg.channel.name
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

    match Players::from_str(&url)
        .ok_or_else(|| String::from("Failed to select extractor for URL"))
    {
        Ok(player) => {
            let mut uris = player.fan_collection(url.as_str()).await?;
            let added = uris.len();
            
            // YTDLP singles don't work.
            // so instead, use the original URI.
            if uris.len() == 1 && player == Players::Ytdl {
                uris.clear();
                uris.push_back(url.clone());
            }
            
            qctx.cold_queue.write().await.extend(uris.drain(..));    

            let maybe_hot = {
                let call = call.lock().await;
                call.queue().len() > 0            
            };

            drop(call); // probably not needed, but just in case
            if !maybe_hot {
                play_routine(qctx.clone()).await?;
            }

            let content = format!(
                "Added {} Song(s) [{}] queued",
                added,
                qctx.cold_queue.read().await.len()
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
        "[{}::{}] skipped track in [{}::{}]",
        msg.author.id, msg.author.name,
        msg.channel.id, msg.channel.name
    );

    let qctx = ctx.data.write().await
        .get_mut::<LazyQueueKey>().unwrap()
        .get_mut(&guild_id).unwrap().clone();
     
    let skipn = args.remains()
        .unwrap_or("1")
        .parse::<isize>()
        .unwrap_or(1);

    if 1 > skipn  {
        msg.channel_id
           .say(&ctx.http, "Must skip at least 1 song")
           .await?;
        return Ok(())
    }

    else if skipn >= qctx.cold_queue.read().await.len() as isize + 1 {
        qctx.cold_queue.write().await.clear();
    }

    else {
        let mut write_lock = qctx.cold_queue.write().await;
        let bottom = write_lock.split_off(skipn as usize - 1);
        write_lock.clear();
        write_lock.extend(bottom);
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

    let cold_queue_len = qctx.cold_queue.read().await.len();

    msg.channel_id
       .say(
            &ctx.http,
            format!("Song skipped [{}]: {} in queue.", skipn, cold_queue_len),
       )
       .await?;

    let mut call = handler_lock.lock().await;
    let queue = call.queue();
    let _ = queue.skip();

    Ok(())
}

#[group]
#[commands(setarl, getarl)]
struct Dangerous;

#[command]
#[only_in(guilds)]
async fn setarl(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    tracing::info!("[{}::{}] set a new arl", msg.author.id, msg.author.name);

    let arl = args.single::<String>()?;

    if !(arl.trim().len() == 192 && arl.chars().all(|c| c.is_ascii_hexdigit())) {
        msg.channel_id.say(&ctx.http, "Invalid ARL").await?;
        return Ok(())
    }

    std::env::set_var("DEEMIX_ARL", arl);
    msg.channel_id.say(&ctx.http, "**ARL has been set**").await?;

    return Ok(())
}

#[command]
#[only_in(guilds)]
async fn getarl(ctx: &Context, msg: &Message) -> CommandResult {
    tracing::info!("[{}::{}] requested arl", msg.author.id, msg.author.name);

    let arl = std::env::var("DEEMIX_ARL");
    tracing::info!("getarl: {:?}", arl);
    match arl {
        Err(e) => { msg.channel_id.say(&ctx.http, format!("Error: {}", e)).await?; }
        Ok(arl) if arl.is_empty() => { msg.channel_id.say(&ctx.http, "ARL not set").await?; }
        Ok(arl) => {
            #[cfg(feature = "check")]
            {
                msg.channel_id.say(&ctx.http, format!("getting arl data...")).await?;
                use serenity::framework::standard::{Args, Delimiter};
                let mut args = Args::new(arl.as_str(), &[Delimiter::Single(' ')]);
                crate::check::arl_check(ctx, msg, args).await?;
            }
            #[cfg(not(feature = "check"))]
            msg.channel_id.say(&ctx.http, format!("ARL: {}", &arl)).await?;
        }
    }
    return Ok(())
}
