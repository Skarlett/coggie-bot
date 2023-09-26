// use crate::{routines::{leave_routine}, deemix};

use serenity::{
    async_trait,
    client::Cache,
    prelude::*,
    
    model::prelude::*, http::Http
};

use songbird::{
    events::{Event, EventContext},
    EventHandler as VoiceEventHandler,
    Songbird,
    Call, 
    create_player,
    input::{
        Input,
        error::Error as SongbirdError,
        Metadata,
        Codec,
        Container,
        children_to_reader
    },
    tracks::{TrackHandle, Track},

    TrackEvent
};

use std::{
    process::Stdio,
    time::{Duration, Instant},
    collections::{VecDeque, HashSet},
    sync::Arc,
    collections::HashMap,
};

use tokio::{
    io::AsyncBufReadExt,
    process::Command,

};

use crate::deemix::{DeemixMetadata, PreloadInput};
use crate::ctrlerror::HandlerError;
use crate::fan::{DeemixUri, YtdlUri};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EventEnd {
    Skipped,
    Finished,
    UnMarked
}

#[derive(Debug, Clone, Copy)]
pub enum TrackAuthor {
    Radio,
    User(UserId)
}

// -> PreTrackRequest<T>   [prefan]
//  
//  -> TrackRequest    [prefan]
//  -> TrackRequestFetched [fanned]
//  -> TrackRequestPreload<T> where T: AudioPlayer
//  -> magic casting .*.*.~ 
//  -> TrackRequestPreload<Box<dyn AudioPlayer>>

//  
// -> TrackRequest         [fan]
// -> DeemixUri/YtdlUri    [audio-player/preload]
// -> PreloadInput/PreloadYtdl [audio-player]
// -> TrackRequestPreload<PreloadInput/PreloadYtdl>
// 
// 
//   -> TrackRequest
//    -> TrackRequestPreload -> TrackRequestPreload<Box<dyn AudioPlayer>>


// trait MetadataTrack<T> {
//     fn raw_metadata(&self) -> T;
// }
pub struct TrackRequest {
    pub author: TrackAuthor,
    pub uri: String,
}

impl TrackRequest {
    pub fn new(uri: String, author: TrackAuthor) -> (uuid::Uuid, Self) {
        (uuid::Uuid::new_v4(), Self {
            author,
            uri
        })
    }
    
    pub fn user(uri: String,  author: UserId) -> (uuid::Uuid, Self) {
        Self::new(
            uri,
            TrackAuthor::User(author)
        )
    }

    pub fn radio(uri: String) -> Self {
        Self::new(
            uri,
            TrackAuthor::Radio,
        )
    }
}

#[derive(Debug, Clone)]
pub struct TrackRequestFetched {
    pub tranid: uuid::Uuid,
    pub author: TrackAuthor,
    pub metadata: MetadataType
}
impl TrackRequestFetched {
    pub fn new(tranid: uuid::Uuid, author: TrackAuthor, metadata: MetadataType) -> Self {
        Self {
            tranid,
            author,
            metadata
        }
    }
}

pub struct TrackRequestPreload<T> {
    pub input: T,
    pub request: TrackRequest
}

impl<T> TrackRequestPreload<T> {
    async fn new(input: T, req: TrackRequest) -> Result<Self, HandlerError>
    where T: Preload<T>, 
          T: Kill
    {
        Ok(Self {
            input: input.preload().await?,
            request: req
        })
    }

    async fn kill(self)
    where T: Kill
    {
        self.input.kill().await;
    }
}

#[async_trait]
impl<T> AudioPlayer for TrackRequestPreload<T>
where T: AudioPlayer + Send {
    async fn load(self) -> Result<(Input, Option<MetadataType>), HandlerError> {
        self.input.load().await
    }
} 

impl<T> From<TrackRequestPreload<T>> for TrackRequestPreload<Box<dyn AudioPlayer>>
where T: AudioPlayer + Send
{
    fn from(x: TrackRequestPreload<T>) -> Self {
        Self {
            input: Box::new(x.input),
            request: x.request
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackRecord {
    // keep this for spotify recommendations
    pub req: TrackRequest,
    pub stop_event: EventEnd,
    pub start: Instant,
    pub end: Instant,
}

pub struct Radio {
    pub suggestions: VecDeque<TrackRequestFetched>,
    pub seeds: VecDeque<MetadataType>,
}

pub struct Queue<T> {


    pub cold: T, 
    
    // VecDeque<TrackRequestFetched>,
    pub warm: VecDeque<TrackRequestPreload<Box<dyn AudioPlayer>>>,
    
    pub has_played: VecDeque<TrackRecord>,
    pub past_transactions: HashMap<uuid::Uuid, TrackRequest>,
    pub transactions_order: VecDeque<uuid::Uuid>,

    pub killed: Vec<std::process::Child>,
    pub radio: Option<Radio>,
}

// Dont break up this structure into smaller pieces
// It is accessed via a global lock, and would require
// multiple open locks to access
// keeping it as one structure reduces the number of locks
pub struct QueueContext {
    pub guild_id: GuildId,
    pub invited_from: ChannelId,
    pub voice_chan_id: GuildChannel,
    pub cache: Arc<Cache>,
 
    // pub data: Arc<RwLock<TypeMap>>,
    pub http: Arc<Http>,
    pub queue: Queue,
    
    //avoid    
    pub manager: Arc<Songbird>,
}

struct Fetched {
    inner: MetadataType,   
    from_request_id: uuid::Uuid,
}


#[derive(Debug, Clone)]
pub enum MetadataType {
    #[cfg(feature = "deemix")]
    Deemix(crate::deemix::DeemixMetadata),
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
            Self::Deemix(meta) => meta.into()
        }
    }
}

#[cfg(feature = "deemix")]
impl From<crate::deemix::DeemixMetadata> for MetadataType {
    fn from(meta: crate::deemix::DeemixMetadata) -> Self {
        Self::Deemix(meta)
    }
}

impl MetadataType {
    pub fn source_url(&self) -> Option<String> {
        match self {
            Self::Standard(meta) => meta.source_url.clone(),            
            #[cfg(feature = "deemix")]
            Self::Deemix(meta) => meta.metadata.source_url.clone()
        }
    }
}

#[async_trait]
pub trait AudioPlayer: Send + Sync {
    async fn load(self) -> Result<(Input, Option<MetadataType>), HandlerError>;
}

#[async_trait]
impl AudioPlayer for DeemixUri {
    async fn load(self) -> Result<(Input, Option<MetadataType>), HandlerError>
    {
        crate::deemix::deemix(&self.0, true).await
            .map_err(HandlerError::from)
            .map(|(input, meta)| (input, meta.map(|x| x.into())))
    }
}

#[async_trait]
impl AudioPlayer for YtdlUri {
    async fn load(mut self) -> Result<(Input, Option<MetadataType>), HandlerError>
    {
        songbird::ytdl(&self.0).await
            .map_err(HandlerError::from)
            .map(|input| (input, None))
    }
}

#[async_trait]
impl AudioPlayer for PreloadInput {
    async fn load(mut self) -> Result<(Input, Option<MetadataType>), HandlerError>
    {
        Ok((Input::new(
            true, 
            children_to_reader::<f32>(self.children.drain(..).collect()),
            Codec::FloatPcm,
            Container::Raw,
        self.metadata.clone().map(|x| x.into())), self.metadata.map(|x| x.into())))
    }
}
///dont pass around Input internally, it doesn't meet the trait bounds
#[async_trait]
impl AudioPlayer for PreloadYtdl {
    async fn load(self) -> Result<(Input, Option<MetadataType>), HandlerError> {
        Ok((self.0, None))
    }
}

#[async_trait]
pub trait Kill {
    async fn kill(self) -> Result<(), HandlerError>;
}

#[async_trait]
pub trait Preload<T> {
    async fn preload(self) -> Result<T, HandlerError>;
}

#[async_trait]
impl Preload<PreloadInput> for DeemixUri {
    async fn preload(self) ->  Result<PreloadInput, HandlerError> {
        crate::deemix::deemix_preload(&self.0).await
            .map_err(HandlerError::from)
    } 
}

/// ytdl doesn't need preloading.
#[async_trait]
impl Preload<YtdlUri> for YtdlUri {
    async fn preload(self) ->  Result<YtdlUri, HandlerError> {
        Ok(self)
    } 
}

#[async_trait]
impl Kill for PreloadInput {
    async fn kill(self) -> Result<(), HandlerError> {
        for mut pid in self.children {
            let _ = pid.kill();
        }       
        Ok(())
    }
}

#[async_trait]
impl Preload<PreloadYtdl> for YtdlUri {
    async fn preload(self) ->  Result<PreloadYtdl, HandlerError> {
        songbird::ytdl(&self.0).await
            .map_err(HandlerError::from)
            .map(|input| PreloadYtdl(input))
    }
}
