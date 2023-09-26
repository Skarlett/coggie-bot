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
    iter::Cycle
};

use tokio::{
    io::AsyncBufReadExt,
    process::Command,

};

use crate::{deemix::deemix_preload};
use crate::deemix::{DeemixMetadata, PreloadInput};
use crate::ctrlerror::HandlerError;
use crate::fan::{DeemixUri, YtdlUri};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EventEnd {
    Skipped,
    Finished,
    UnMarked
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone)]
pub struct TrackRequest {
    pub tranid: uuid::Uuid,
    pub author: TrackAuthor,
    pub uri: String,

}

impl TrackRequest {
    pub fn new(uri: String, author: TrackAuthor) -> Self {
        Self {
            tranid: uuid::Uuid::new_v4(),
            author,
            uri
        }
    }
    
    pub fn user(uri: String,  author: UserId) -> Self {
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
    pub author: TrackAuthor,
    pub metadata: MetadataType,
    pub track_request: TrackRequest,
}

impl TrackRequestFetched {
    pub fn new(track_request: TrackRequest, author: TrackAuthor, metadata: MetadataType) -> Self {
        Self {
            track_request,
            author,
            metadata
        }    
    }
    
    pub async fn into_preload(self) -> TrackRequestPreload<Box<dyn AudioPlayer>> {
        match self.metadata {
            MetadataType::Deemix(x) => {
                    let x = TrackRequestPreload::new(
                        x.preload().await.unwrap(),
                        self.track_request
                    );

                    todo!()
            }
            MetadataType::Standard(x) => todo!()
        }
    }
}

pub struct TrackRequestPreload<T> {
    pub input: T,
    pub request: TrackRequest
}

impl<T> TrackRequestPreload<T> {
    fn new(input: T, req: TrackRequest) -> Result<Self, HandlerError>
    {
        Ok(Self {
            input,
            request: req
        })
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
where T: AudioPlayer + Send + 'static
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

pub struct QueueHistory {
    pub has_played: VecDeque<TrackRecord>,
    pub past_transactions: HashMap<uuid::Uuid, TrackRequest>,
    pub transactions_order: VecDeque<uuid::Uuid>,
    pub killed: Vec<std::process::Child>,
}


pub struct Queue {
    // UserQueue, Radio
    pub cold: Box<dyn QueueStrategy>,
    pub radio: Box<dyn QueueStrategy>,
    pub past: QueueHistory,
    pub warm: VecDeque<TrackRequestPreload<Box<dyn AudioPlayer>>>,
}


impl Queue {
    pub fn warm_track(&mut self, strats: &[ Box<dyn QueueStrategy> ]) {
        for strat in strats {
            // if let Some(track) = strat.next_track() {
                // self.warm.push_back(track.preload());
                // return
            // }
        }
    }
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
impl Preload<PreloadInput> for DeemixMetadata {
    async fn preload(self) ->  Result<PreloadInput, HandlerError> {
        crate::deemix::deemix_preload(&self.metadata.source_url.unwrap()).await
            .map_err(HandlerError::from)
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

enum InnerQueueStage {
    Before,
    After(TrackRequestFetched),
}

// struct RadioStrategyLast<Q>
// {
    
// } 

// impl RadioStrategy<InnerQueueStage> for RadioStrategyLast {
//     fn act(&mut self, event: InnerQueueStage) -> Option<TrackRequestFetched> {
//         match event {
//             InnerQueueStage::Before => {}
//             InnerQueueStage::After(track) => {                
//                 let mut queue = self.qctx.queue.cold.write().await;
//                 queue.radio_next = Some(track);
//             }
//         }
//         None
//     }
// }



trait RadioStrategy {
    fn act(&mut self) -> Option<TrackRequestFetched> { None }
}

/// Items moved from cold to warm queue
/// Whenever QueueStrategy returns None,
/// RadioStrategy<T> is called with the last
trait QueueStrategy: Sync + Send {
    /// Coming out of cold queue into warm
    fn next_track(&mut self) -> Option<TrackRequestFetched>;
    
    
    /// Coming into cold queue
    fn add_tracks(&mut self, tracks: &[TrackRequestFetched]) -> bool;



    /// remove tracks from cold queue
    fn remove_tracks(&mut self, tracks: &[TrackRequestFetched]) {}
}

struct TraditionalQueue {
    list: VecDeque<TrackRequestFetched>
}

impl QueueStrategy for TraditionalQueue {
    fn next_track(&mut self) -> Option<TrackRequestFetched> {
        self.list.pop_front()
    }

    fn add_tracks(&mut self, tracks: &[TrackRequestFetched]) -> bool {
        
        let mut new = VecDeque::from(
            tracks.iter()
                .cloned()
                .collect::<Vec<_>>()
        );
        
        self.list.append(&mut new);

        
        true
    }
} 

struct RoundRobinQueue {
    lookup: HashMap<UserId, VecDeque<TrackRequestFetched>>,
    turns: Vec<UserId>,
    position: usize,
}

// remove all items from cycle
// maintaing the current order,
// and append new user to the end
fn update_cyclic<I>(cyclic: &mut Cycle<I>, new_user: UserId )
where 
    I: Iterator<Item = UserId> + Clone
{
 
}

impl QueueStrategy for RoundRobinQueue
{
    fn next_track(&mut self) -> Option<TrackRequestFetched> {
        let first_user = self.turns.len() % self.position;
        todo!()
        // let mut user = first_user;

        // loop {
        //     if self.map.is_empty() { return None }

        //     let track = self.map.get_mut(&user)
        //         .unwrap()
        //         .pop_front();   
            
        //     if let None = track {
        //         self.map.remove(&user);
        //     }
            
        //     else {
        //         return track;
        //     }

        //     user = self.cyclic.next().expect("inf iter");
        // }
    }

    fn add_tracks(&mut self, tracks: &[TrackRequestFetched]) -> bool {
        // let mut reconstruct_cycler = false;        

        // for track in tracks
        // {        
        //     let uid = match track.author {
        //         TrackAuthor::User(uid) => uid,                 
        //         TrackAuthor::Radio => unreachable!()
        //     };
            
        //     self.map.entry(uid) 
        //         .or_insert_with(|| {
        //             reconstruct_cycler = true;
        //             VecDeque::new()
        //         })
        //         .push_back(TrackRequestFetched::new(
        //             track.track_request,
        //             track.author,
        //             track.metadata.clone()
        //         ));            
        // }
        todo!()
    }
}