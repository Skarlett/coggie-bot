// this is the rat nest
// be prepared
// to see how lazy i can be.
use serenity::{
    async_trait, client::Cache, framework::standard::{
        macros::{command, group}, Args, CommandResult
    }, http::Http, json, model::{channel::Message, prelude::*}, prelude::*, FutureExt
};

use songbird::{
    create_player, error::{JoinError, JoinResult}, events::{Event, EventContext, EventData}, input::{
        error::Error as SongbirdError, Input, Metadata
    }, tracks::{PlayMode, Track, TrackHandle}, Call, EventHandler as VoiceEventHandler, Songbird, TrackEvent
};

use std::{
    process::Stdio,
    time::{Duration, Instant},
    collections::VecDeque,
    sync::Arc,
    collections::HashMap,
    path::PathBuf,
};

use std::sync::atomic::AtomicBool;
use tokio::{
    io::AsyncBufReadExt,
    process::Command,
    sync::oneshot::Sender

};
use parking_lot::{Mutex, MutexGuard};

use tokio::io::AsyncWriteExt;
use serenity::futures::StreamExt;
use songbird::input::cached::Compressed;


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EventEnd {
    Skipped,
    Finished,
    UnMarked
}

pub type LazyQueue = HashMap<GuildId, Arc<QueueContext>>;
pub struct LazyQueueKey;
impl TypeMapKey for LazyQueueKey {
    type Value = LazyQueue;
}

#[derive(Debug, Clone)]
pub(crate) struct TrackRecord {
    // keep this for spotify recommendations
    pub metadata: MetadataType,
    pub stop_event: EventEnd,
    pub start: Instant,
    pub end: Instant,
}

pub struct ColdQueue {
    pub queue: VecDeque<String>,
    pub has_played: VecDeque<TrackRecord>,
    pub use_radio: bool,
    pub queue_next: Option<(Compressed, Option<MetadataType>)>,
    pub crossfade_lhs: Option<TrackHandle>,
    pub crossfade_rhs: Option<TrackHandle>,
    // urls
    pub radio_queue: VecDeque<String>,
    pub radio_next: Option<(Compressed, Option<MetadataType>)>,
}

struct GuildConfig {
    pub crossfade: bool,
    pub use_radio: bool,
}

pub struct QueueContext {
    pub guild_id: GuildId,
    pub invited_from: ChannelId,
    pub voice_chan_id: GuildChannel,
    pub cache: Arc<Cache>,
    pub data: Arc<RwLock<TypeMap>>,
    pub http: Arc<Http>,
    pub manager: Arc<Songbird>,
    pub cold_queue: Arc<RwLock<ColdQueue>>,
    pub crossfade: AtomicBool,
    pub crossfade_step: Mutex<i32>
}

#[derive(Debug, Clone)]
pub(crate) enum MetadataType {
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
