// this is the rat nest
// be prepared
// to see how lazy i can be.
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
    input::{
        Input,
        error::Error as SongbirdError,
        Metadata,
    },
    // queue::TrackQueue,
    tracks::TrackHandle,
    TrackEvent
};

use std::{
    process::Stdio,
    time::{Duration, Instant},
    collections::VecDeque,
    sync::Arc,
    collections::HashMap,
    path::PathBuf,
};

use tokio::{
    io::AsyncBufReadExt,
    process::Command,

};
use parking_lot::Mutex;

use tokio::io::AsyncWriteExt;
use serenity::futures::StreamExt;
use songbird::input::cached::Compressed;


use cutils::{availbytes, bigpipe, max_pipe_size};

#[cfg(feature = "deemix")]
use crate::deemix::{DeemixMetadata, _deemix};

#[group]
#[commands(join, leave, queue, now_playing, skip, list, play_source)]
pub struct BetterPlayer;

#[group]
#[commands(seed, radio)]
pub struct Radio;


const TS_PRELOAD_OFFSET: Duration = Duration::from_secs(20);
const TS_CROSSFADE_OFFSET: Duration = Duration::from_secs(10);
const TS_ABANDONED_HB: Duration = Duration::from_secs(720);
const HASPLAYED_MAX_LEN: usize = 10;


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum EventEnd {
    Skipped,
    Finished,
    UnMarked
}

type LazyQueue = HashMap<GuildId, Arc<QueueContext>>;
pub struct LazyQueueKey;
impl TypeMapKey for LazyQueueKey {
    type Value = LazyQueue;
}

#[derive(Debug, Clone)]
struct TrackRecord {
    // keep this for spotify recommendations
    metadata: MetadataType,
    stop_event: EventEnd,
    start: Instant,
    end: Instant,
}

struct ColdQueue {
    pub queue: VecDeque<String>,
    pub has_played: VecDeque<TrackRecord>,
    pub use_radio: bool,
    // urls
    pub radio_queue: VecDeque<String>,
    pub radio_next: Option<(Compressed, Option<MetadataType>)>,
}

pub struct QueueContext {
    crossfade: Duration,
    guild_id: GuildId,
    invited_from: ChannelId,
    voice_chan_id: GuildChannel,
    cache: Arc<Cache>,
    data: Arc<RwLock<TypeMap>>,
    http: Arc<Http>,
    manager: Arc<Songbird>,
    cold_queue: Arc<RwLock<ColdQueue>>,
    hot: Arc<Mutex<VecDeque<TrackHandle>>>
}

#[derive(Debug, Clone)]
enum MetadataType {
    #[cfg(feature = "deemix")]
    Deemix(crate::deemix::DeemixMetadata),
    Disk(PathBuf),
    Standard(Metadata),
}

impl From<Metadata> for MetadataType {
    fn from(meta: Metadata) -> Self {
        Self::Standard(meta)
    }
}

impl Into<Metadata> for MetadataType {
    fn into(self) -> Metadata {
        match self {
            Self::Standard(meta) => meta,
 
            #[cfg(feature = "deemix")]
            Self::Deemix(meta) => meta.into(),


            #[cfg(feature = "http-get")]
            Self::Disk(fp) => Metadata { source_url: fp.into_os_string().into_string().ok(), ..Default::default() },
        }
    }
}

#[cfg(feature = "deemix")]
impl From<crate::deemix::DeemixMetadata> for MetadataType {
    fn from(meta: crate::deemix::DeemixMetadata) -> Self {
        Self::Deemix(meta)
    }
}

async fn seed_from_history(has_played: &VecDeque<TrackRecord>) -> std::io::Result<VecDeque<String>> {
    let seeds =
        has_played
            .iter()
            // Don't include skipped tracks
            .filter(|x| x.stop_event != EventEnd::Skipped)
            .filter_map(|x|
                match &x.metadata {
                    MetadataType::Deemix(meta) => meta.isrc.clone(),
                    _ => None
                })
            .collect::<Vec<_>>();


    if seeds.is_empty() {
        return Ok(seeds.into());
    }

    return recommend(&seeds, 5).await;

}

async fn preload_radio_track(
    cold_queue: &mut ColdQueue
) -> Result<(), String> {
    // pop seeds in radio
    let mut tries = 5;
    // attempts/tries loop
    loop {
        let uri = match cold_queue.radio_queue.pop_front() {
            Some(x) => Some(x),
            None => {
                cold_queue.radio_queue.clear();
                cold_queue.radio_queue.extend(seed_from_history(&cold_queue.has_played).await.unwrap_or_else(|_| VecDeque::new()));
                cold_queue.radio_queue.pop_front()
            }
        };

        if let Some(uri) = uri {
            match _deemix(&uri, &[], false).await {
                Ok((preload_input, metadata)) => {
                        cold_queue.radio_next = Some((Compressed::new(
                            preload_input,
                            songbird::driver::Bitrate::BitsPerSecond(128_000)
                        ).unwrap(),

                        metadata.map(|x| x.into())
                    ));
                    return Ok(())
                }

                Err(why) =>  {
                    tries -= 1;
                    tracing::error!("Error preloading radio track: {}", why);
                    if 0 >= tries {
                        return Err("Exceeded max tries".to_string());
                    }
                    continue
                }
            }
        }
        return Err("Fall through".to_string());
    }
}

async fn play_preload_radio_track(
    call: &mut Call,
    radio_preload: Compressed,
    metadata: Option<MetadataType>,
    qctx: Arc<QueueContext>
)
{
    let preload_result = Players::play_preload(call, radio_preload.new_handle().into(), metadata).await;
    match preload_result {
        Err(why) =>{
            tracing::error!("Failed to play radio track: {}", why);
        }
        Ok((handle, _)) => handle.add_event(
            Event::Delayed(
                handle.metadata()
                      .duration
                      .unwrap()
                    - TS_PRELOAD_OFFSET
            ),
            PreloadInvoker(qctx.clone()),
        ).unwrap()
    }
}

struct RadioInvoker(Arc<QueueContext>);
#[async_trait]
impl VoiceEventHandler for RadioInvoker {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        if let Some(call) = self.0.manager.get(self.0.guild_id) {
            let mut call = call.lock().await;
            let mut cold_queue = self.0.cold_queue.write().await;

            // `PreloadInvoker` may have placed a track (from the user queue)
            // before this event was fired.
            // If true, we clear our trackers.
            if let Some(_current_track_handle) = call.queue().current() {
                // do nothing
            }

            // `PreloadInvoker` has not placed anything,
            // lets fire it's routine on our thread.
            else if let Ok(true) = invoke_cold_queue(&mut call, &mut cold_queue, self.0.clone()).await {
                // do nothing.
            }

            // If all else fails, play the preloaded track on radio
            else if cold_queue.use_radio {
                // if the user queue is empty, try the preloaded radio track
                if let Some((radio_preload, metadata)) = cold_queue.radio_next.take() {
                    play_preload_radio_track(&mut call, radio_preload, metadata, self.0.clone()).await;
                    let _ = preload_radio_track(&mut cold_queue).await;
                    return None;
                }
            }

            cold_queue.radio_next = None;
            let _ = preload_radio_track(&mut cold_queue).await;
        }
        None
    }
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

struct PreloadInvoker(Arc<QueueContext>);
#[async_trait]
impl VoiceEventHandler for PreloadInvoker {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {      
        if let Some(call) = self.0.manager.get(self.0.guild_id) {
            let mut call = call.lock().await;
            let mut cold_queue = self.0.cold_queue.write().await;
            let _ = invoke_cold_queue(&mut call, &mut cold_queue, self.0.clone()).await;
        }
        None
    }
}

#[allow(unused_variables)]
#[derive(Debug)]
pub enum HandlerError {
    Songbird(SongbirdError),
    IOError(std::io::Error),
    Serenity(serenity::Error),

    #[cfg(feature = "http-get")]
    Reqwest(reqwest::Error),

    #[cfg(feature = "http-get")]
    UnsupportedMediaType(String),

    #[cfg(feature = "deemix")]
    DeemixError(crate::deemix::DeemixError),

    WrongMetadataType,

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

#[cfg(feature = "http-get")]
impl From<reqwest::Error> for HandlerError {
    fn from(err: reqwest::Error) -> Self {
        HandlerError::Reqwest(err)
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

            Self::WrongMetadataType
                => write!(f, "Programming bug, got a different MetadataType than expected"),

            #[cfg(feature = "http-get")]
            Self::UnsupportedMediaType(content_type)
                => write!(f, "Content type is not supported [{}]", content_type),

            #[cfg(feature = "http-get")]
            Self::Reqwest(err)
                => write!(f, "Reqwest error: {}", err),

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
async fn fan_deezer(uri: &str, buf: &mut VecDeque<String>) -> Result<usize, HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(not(feature="ytdl"))]
async fn fan_ytdl(_uri: &str, _buf: &mut VecDeque<String>) -> Result<usize, HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(feature = "http-get")]
async fn ph_httpget_player(
    uri: &str,
    guild_id: u64,
    ref_fp: &mut PathBuf,
) -> (Result<(Input, Option<MetadataType>), HandlerError>) {
    tracing::info!("[HTTP-GET] Downloading: {}", uri);

    // let fp = tempfile::tempfile()?;
    use rand::Rng;
    let id: String = (0..12)
        .map(|_| char::from(rand::thread_rng().gen_range(97..123)))
        .collect();

    let fp = std::env::temp_dir()
        .join("coggiebot")
        .join(guild_id.to_string());

    match tokio::fs::create_dir_all(&fp).await {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Failed to create temp dir: {}", e);
            return (Err(HandlerError::IOError(e)));
        }
    }
    let fp = fp.join(format!("{}", id));

    match get_file(uri, guild_id, &fp).await.map_err(HandlerError::from) {
        Ok(input) => Ok((input, Some(MetadataType::Disk(fp.clone())))),
        Err(e) => {
            if let Ok(true) = tokio::fs::try_exists(&fp).await {
                tokio::fs::remove_file(&fp).await;
            }
            Err(e)
        }
    }
}

#[cfg(feature = "deemix")]
async fn ph_deemix_player(uri: &str) -> Result<(Input, Option<MetadataType>), HandlerError> {
    crate::deemix::deemix(uri).await
        .map_err(HandlerError::from)
        .map(|(input, meta)| (input, meta.map(|x| x.into())))   
    }

#[cfg(feature = "ytdl")]
async fn ph_ytdl_player(uri: &str) -> Result<(Input, Option<MetadataType>), HandlerError> {
    return songbird::ytdl(uri).await.map_err(HandlerError::from)
        .map(|input| (input, None))
}

#[cfg(not(feature = "deemix"))]
struct FakeMeta(Metadata);

#[cfg(not(feature = "deemix"))]
impl Into<Metadata> for FakeMeta {
    fn into(self) -> Metadata {
        self.0
    }
}

#[cfg(not(feature = "deemix"))]
async fn ph_deemix_player(uri: &str) -> Result<(Input, Option<FakeMeta>), HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(not(feature = "ytdl"))]
async fn ph_ytdl_player(_uri: &str) -> Result<(Input, Option<MetadataType>), HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(not(feature = "http-get"))]
async fn ph_httpget_player(_uri: &str) -> (PathBuf, Result<Input, HandlerError>) {
    return (PathBuf::new(), Err(HandlerError::NotImplemented))
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
        let json =
            serde_json::from_str(&line).unwrap();
        buf.push(json);
    } 
    Ok(())
}

async fn recommend(isrcs: &Vec<String>, limit: u8) -> std::io::Result<VecDeque<String>> {
    let mut buffer = std::collections::HashSet::new();

    tracing::info!("running spotify-recommend -l {} {}", limit, isrcs.join(" "));
    let recommend = tokio::process::Command::new("spotify-recommend")
        .arg("-l")
        .arg(format!("{}", limit))
        .args(isrcs.iter())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    let output = recommend.wait_with_output()
        .await?;
    
    let mut lines = output.stdout.lines();
    
    while let Some(x) = lines.next_line().await? {
        buffer.insert(x);
    }
    tracing::info!("spotify-stream finished [{}]", buffer.len());
    let mut ret = VecDeque::new();
    for x in buffer {
        ret.push_back(x);   
    }
    Ok(ret)
}

#[derive(PartialEq, Eq)]
pub enum Players {
    Ytdl,
    Deemix,
    HttpGet,
}

impl Players {
    fn from_str(data : &str) -> Option<Self>
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

    async fn into_input(&self, uri: &str, guild_id: GuildId) -> Result<(Input, Option<MetadataType>), HandlerError>
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
                        // return Ok((track_handle, None))
                    }

                    Err(e) => {
                        // cleanup(fp)
                        // TODO FIXME
                        return Err(e)
                    }
                }
                result
            }
        }
    }

    // async fn play(&self, handler: &mut Call, uri: &str, guild_id: GuildId) -> Result<(TrackHandle, Option<MetadataType>), HandlerError>
    // {
    //     let (input, metadata) = self.into_input(uri, guild_id).await?;
    //     let (track, track_handle) = create_player(input);
    //     handler.enqueue(track);
    //     Ok((track_handle, metadata))
    // }

    async fn play_preload(
        handler: &mut Call,
        preload: Input, // &mut Vec<std::process::Child>,
        metadata: Option<MetadataType>
    )
    -> Result<(TrackHandle, Option<MetadataType>), HandlerError>
    {
        let (track, track_handle) = create_player(preload);
        handler.enqueue(track);
        Ok((track_handle, metadata
            //TODO: FIXME!: preload.metadata.map(|x| x.into())
        ))
    }

    async fn fan_collection(&self, uri: &str) -> Result<VecDeque<String>, HandlerError> {
        let mut buf = VecDeque::new();
        match self {
            Self::HttpGet => {buf.push_back(uri.to_owned()); Ok(1)},
            Self::Deemix => fan_deezer(uri, &mut buf).await,
            Self::Ytdl => fan_ytdl(uri, &mut buf).await 
        }?;

        return Ok(buf)
    }
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
    use songbird::input::Metadata;

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

struct RemoveTempFile(PathBuf);
#[async_trait]
impl VoiceEventHandler for RemoveTempFile {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let _ = tokio::fs::remove_file(&self.0).await;
        None
    }
}

async fn invoke_cold_queue(
    call: &mut Call,
    cold_queue: &mut ColdQueue,
    qctx_arc: Arc<QueueContext>
) -> Result<bool, HandlerError> {
    let mut tries = 4;

    while let Some(uri) = cold_queue.queue.pop_front() {
        tracing::info!("Now playing: {}", uri);
        let player = Players::from_str(&uri)
            .ok_or_else(|| HandlerError::NotImplemented)?;

        // turn realization to live
        match player.into_input().await
        {
            Ok((input, metadata)) => {
                // let (track, track_handle) = create_player(input);
                let track = enqueue(input).await?;

                if let Some(duration) = track.metadata().duration {
                    if duration < TS_PRELOAD_OFFSET {
                        tracing::warn!("No duration provided, preloading disabled");
                        break
                    }

                    tracing::info!("Preload Event Added from Duration");
                    track.add_event(
                        Event::Delayed(duration - TS_PRELOAD_OFFSET),
                        PreloadInvoker(qctx_arc.clone())
                    ).unwrap();
                }

                if cold_queue.has_played.len() > HASPLAYED_MAX_LEN {
                    let _ = cold_queue.has_played.pop_back();
                }

                // --- START
                // This portion of code marks songs as finished or not.
                // Under normal circumstances, this would be placed on the "EndTrack"
                // Event. It also happens that pausing, skipping, and leaving
                // all cause this event to fire.
                // So instead, its placed here to avoid those.
                if let Some(x) = cold_queue.has_played.front_mut() {
                    if let EventEnd::UnMarked = x.stop_event {
                        x.stop_event = EventEnd::Finished;
                        x.end = Instant::now();
                    }
                }

                let data = TrackRecord {
                    metadata: metadata.unwrap_or(MetadataType::from(track.metadata().clone())),
                    stop_event: EventEnd::UnMarked,
                    start: Instant::now(),
                    end: Instant::now(),
                };

                cold_queue.has_played.push_front(data);
                // --- END

                // Preemptively load the next audio track
                // `TS_PRELOAD_OFFSET` seconds before this `track`
                // ends.
                track.add_event(
                    Event::Delayed(track.metadata().duration.unwrap() - TS_PRELOAD_OFFSET),
                    PreloadInvoker(qctx_arc)
                ).unwrap();

                return Ok(true);
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
    Ok(false)
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

    match gchan.bitrate {
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
                crossfade: Duration::from_secs(0),
                invited_from: msg.channel_id,
                cache: ctx.cache.clone(),
                data: ctx.data.clone(),
                manager: manager.clone(),
                http: ctx.http.clone(),
                cold_queue: Arc::new(RwLock::new(ColdQueue {
                    queue: VecDeque::new(),
                    has_played: VecDeque::new(),
                    use_radio: false,
                    radio_next: None,
                    radio_queue: VecDeque::new(),
                })),
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
        RadioInvoker(queuectx.clone())
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

            // --- START
            // WARNING: removing these curly braces will cause a deadlock.
            // amount of hours spent on this: 5
            {
                qctx.cold_queue.write().await.queue.extend(uris.drain(..));

                // check for hot loaded track
                let hot_loaded = {
                    let call = call.lock().await;
                    call.queue().len() > 0
                };


                let mut call = call.lock().await;
                let mut cold_queue = qctx.cold_queue.write().await;
                if hot_loaded == false {
                    invoke_cold_queue(&mut call, &mut cold_queue, qctx.clone()).await?;
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

    match manager.get(guild_id) {
        Some(call) => {
            let call = call.lock().await;
            let queue = call.queue();
            let _ = queue.skip();
        }
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
            format!("Song skipped [{}]: {} in queue.", skipn, skipn-cold_queue_len as isize),
       )
       .await?;

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
/// @bot seed [on/off/(default: status)/uri]
async fn seed(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
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
    let act = args.remains()
        .unwrap_or("status");

    match act {
        "status" =>
            { msg.channel_id
                .say(
                    &ctx.http,
                    qctx.cold_queue.read().await.radio_queue.clone().into_iter().collect::<Vec<_>>().join("\n")
                ).await?; },

        _ => {}
    }

    Ok(())
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

#[command]
#[only_in(guilds)]
/// @bot radio [on/off/(default: status)]
async fn radio(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
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
    let act = args.remains()
        .unwrap_or("status");

    match act {
        "status" =>
            { msg.channel_id
                .say(
                    &ctx.http,
                    if qctx.cold_queue.read().await.use_radio
                    { "on" } else { "off" },
                ).await?; },

        "on" => {
            qctx.cold_queue.write().await.use_radio = true;
            msg.channel_id
               .say(&ctx.http, "Radio enabled")
               .await?;
        }
        "off" => {
            let mut lock = qctx.cold_queue.write().await;
            lock.radio_queue.clear();
            lock.use_radio = false;

            msg.channel_id
               .say(&ctx.http, "Radio disabled")
               .await?;
        }
        _ => {}
    }
    Ok(())
}


#[command]
#[only_in(guilds)]
/// @bot radio [on/off/(default: status)]
async fn play_source(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
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

    let player = Players::from_str(&url).unwrap();
    let mut call = handler.lock().await;
    let guild = msg.guild(&ctx.cache).unwrap();

    let (input, metadata) = player.into_input(&url, guild.id).await?;
    call.play_source(input);
    // handler.enqueue(track);
    // Ok((track_handle, metadata))
    Ok(())
}

struct CrossFadeVolume {
    step_by: f32,
    sum: f32,
    start: f32,
    limit: f32
}

impl Iterator for CrossFadeVolume {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.sum > self.limit {
            return None
        }
        self.sum += self.step_by;
        Some(self.sum)
    }
}

struct CrossFadeInvoker {
    qctx: Arc<QueueContext>,
    iter: CrossFadeVolume,
    next_on: bool
}

impl CrossFadeInvoker {
    fn new(ctx: Arc<QueueContext>) -> Self {
        Self {
            ctx,
            next_flip: false,
            iter: CrossFadeVolume {
                step_by: 0.01,
                sum: 0,
                start: 0,
                limit: 100
            }
        }
    }
}

#[async_trait]
impl VoiceEventHandler for CrossFadeInvoker {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        if let None = self.0.manager.get(self.0.guild_id) {
            return None
        }

        let mut call = self.qctx.manager.get().lock().await;
        let mut cold_queue = self.qctx.cold_queue.write().await;

        let (current, next) = {
            let lock = self.qctx.hot.lock();
            let mut current = hot_lock.nth(1).clone();
            let mut next = hot_lock.nth(2).clone();
            (current, next)
        };

        match self.iter.next() {
            Some(volume) => {
                let volume = 100.0 - volume;
                let next_volume = volume;

                if let Some(next) = next {
                    next.set_volume(volume);
                    next.play();
                }

                if 1e-1 >= volume {
                    current_track.stop();
                }

                Some(Duration::from_millis(100));
            }

            None => {
                let _ = self.qctx.hot.pop_front();
                None
            }
        }
    }
}

async fn enqueue_source(
    call: &mut Call,
    input: Input,
    crossfade: bool,
    qctx: Arc<QueueContext>
) -> Result<TrackHandle, HandlerError>  {

    if ! crossfade {
        let track_handle = driver.enqueue(input);
        return
    }

    match manager.get(guild_id) {
        Some(call) => {
            let call = call.lock().await;
            let queue = call.queue();
            let _ = queue.skip();
        }
        None => {
            msg.channel_id
               .say(&ctx.http, "Not in a voice channel to play in")
               .await?;
            return Ok(())
        }
    };

    let (track, handle) = create_player(input)
        .map_err(HandlerError::from)?;

    handle.add_event(
        Event::Delayed(duration - TS_CROSSFADE_OFFSET),
        CrossFadeInvoker(qctx_arc.clone())
    ).unwrap();

    {
        let cold_queue = qctx.cold_queue.lock();
        if cold_queue.is_empty() {
            handle.play();

            qctx
                .hot
                .lock()
                .push_front(handle)
        }
        else {
            cold_queue.queue.push()
        }
    }
}
