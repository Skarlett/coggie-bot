
use serenity::{
    async_trait,
    model::channel::Message,
    framework::standard::{
        macros::{command, group},
        CommandResult, Args,
    }, 
    client::Cache,
    prelude::*,
    
    model::prelude::*, http::{Http, request::Request}, json
};

use serenity::futures::StreamExt;

use songbird::{
    error::{JoinResult, JoinError},
    events::{Event, EventContext},
    EventHandler as VoiceEventHandler,
    Songbird,
    Call, 
    create_player,
    tracks::{TrackHandle, Track},

    TrackEvent
};

use std::{
    time::{Instant},
    collections::{VecDeque, HashSet},
    sync::Arc,
};

use std::iter::Cycle;

use cutils::{availbytes, bigpipe, max_pipe_size};

use crate::{deemix::{DeemixMetadata, PreloadInput, DeemixError}, player::{TrackRequestPreload, TrackRequestFetched}};

use crate::player::{Queue, AudioPlayer, MetadataType, QueueContext, TrackRecord, EventEnd, TrackRequest};
use crate::ctrlerror::HandlerError;


// async fn next_track(queue: &mut VecDeque<TrackRequestPreload<Box<dyn AudioPlayer + Send>>> )
//     -> Option<Box<dyn AudioPlayer + Send>> {
    
//     let (mut radio, mut user): (VecDeque<_>, VecDeque<_>) = queue.iter_mut().partition(|x|
//         if let crate::player::TrackAuthor::User(_) = &x.request.author {
//             true
//         } else {
//             false
//         }
//     );
    
//     let x = user.pop_front();
    

//     if x.is_some() {
//         return x;
//     }

//     let x = radio.pop_front();
//     if x.is_some() {
//         return x;
//     }
 
//     None
// }

// pub async fn play_once_routine(
//     req: TrackRequest,
//     has_played: &mut VecDeque<TrackRecord>
// ){
//     has_played.push_front(
//         TrackRecord {
//             start: Instant::now(),
//             end: Instant::now(),
//             stop_event: EventEnd::UnMarked,
//             req
//         }
//     );
// }


// pub async fn play_queue_routine(qctx: Arc<QueueContext>) -> Result<bool, HandlerError> {
//     let mut tries = 4;
    
//     let handler = qctx.manager.get(qctx.guild_id)
//         .ok_or_else(|| HandlerError::NoCall)?;
    
//     let mut call = handler.lock().await;

//     while let Some((loader, requester)) = next_track(&mut qctx.handle ).await {
//         match loader.load().await {
//             Ok((input, metadata)) => {

//                 // before_play();

//                 let (track, trackhandle) = create_player(input);
//                 call.enqueue(track);
//                 return Ok(true);
//             },
//             Err(why) => {
//                 tries -= 1;
//                 qctx.invited_from.send_message(&qctx.http, |m| {
//                     m.content(format!("Error loading track: {}", why))
//                 }).await?;

//                 if 0 >= tries {
//                     return Err(why);
//                 }
//             }
//         }

//         let (track, trackhandle) = create_player(input);
        
//         call.enqueue(track);
//     };
    
//     Ok(true)
// }

// pub async fn leave_routine (
//     data: Arc<RwLock<TypeMap>>,
//     guild_id: GuildId,
//     manager: Arc<Songbird>
// ) -> JoinResult<()>
// {   
//     let handler = manager.get(guild_id).unwrap();

//     {
//         let mut call = handler.lock().await;
//         call.remove_all_global_events();
//         call.stop();
//     }    
    
//     manager.remove(guild_id).await?;

//     {
//         let mut glob = data.write().await; 
//         let queue = glob.get_mut::<crate::LazyQueueKey>()
//             .expect("Expected LazyQueueKey in TypeMap");
//         queue.remove(&guild_id);
//     }

//     Ok(())
// }

// pub async fn radio_routine(queue: &mut VecDeque<MetadataType>) 
// -> (Option<PreloadInput>, Vec<HandlerError>) {
//     let mut errors = Vec::new();
//     while let Some(meta) = queue.pop_front() {
//         let mut tries = 5;   

//         if let None = meta.source_url() {
//             continue
//         }

//         match crate::deemix::deemix_preload(&meta.source_url().unwrap()).await {
//             Ok(preload_input) => return ( Some(preload_input), errors),
//             Err(why) =>  {
//                 tries -= 1;

//                 if 0 >= tries {
//                     break;
//                 }
//                 tracing::error!("Error preloading radio track: {}", why);
//                 errors.push(HandlerError::DeemixError(why));
//             }
//         }
//     }
//     (None, errors)
// }


// use crate::deemix::SpotifyRecommendError;
// pub async fn after_enqueue(
//     qctx: Arc<QueueContext>,
// ) -> Result<(), SpotifyRecommendError> {   
    
//     let mut queue = qctx.queue; 
//     let pipesize = max_pipe_size().await.expect("Failed to get pipe size");
    
//     if let Some(radio) = queue.radio {
//         let urls = radio.seeds
//             .iter()
//             .filter_map(|x| match x {
//                 MetadataType::Deemix(meta) => Some(meta.isrc.unwrap()),
//                 _ => None
//             })
//             .join(" ");
    
//         let generated = crate::deemix::recommend(urls, 5).await.unwrap();
//         if generated.is_empty() { return Err(SpotifyRecommendError::BadSeeds) }

//         let generated = queue.queue.pop_front();
//         generated.unwrap()
//     }

//     Ok(())
// }

// async fn load_userqueue() -> Result<(), HandlerError> {
    
    
//     todo!()
// }


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
           let _ = msg.reply(
               &ctx.http,
               r#"**Couldn't detect bitrate.** For the best experience,
check that the voice room is using 128kbps."#).await;
       }
       Some(x) => {

            #[cfg(feature = "deemix")]
            let _ = msg.reply(
                &ctx,
                format!(
                    r#"**Low quality voice channel** detected.
For the best experience, use 128kbps, & spotify links
[Currently: {}kbps]"#,  (x / 1000))
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
                // data: ctx.data.clone(),
                manager: manager.clone(),
                http: ctx.http.clone(),
                queue: todo!() 
                
                
                // Arc::new(RwLock::new(crate::player::Queue {
                //     cold: VecDeque::new(),
                //     warm: VecDeque::new(),
                //     radio: None,
                //     has_played: VecDeque::new(),
                //     past_transactions: HashMap::new(),
                //     transactions_order: VecDeque::new(),
                //     killed: Vec::new(),
                // })),
                // sfx: Arc::new(RwLock::new(todo!())),

            }
        } else {
            tracing::error!("Expected voice channel (GuildChannel), got {:?}", chan);
            return Err(JoinError::NoCall);
        };

    
    let queuectx = Arc::new(queuectx);
    
    {
        let mut glob = ctx.data.write().await; 
        let queue = glob.get_mut::<crate::LazyQueueKey>()
            .expect("Expected LazyQueueKey in TypeMap");
        queue.insert(guild_id, queuectx.clone());
    }

    let _ = call.deafen(true).await;
    
    // call.add_global_event(
    //     Event::Track(TrackEvent::End),
    //     crate::player::TrackEndLoader(queuectx.clone())
    // );
    
    // call.add_global_event(
    //     Event::Periodic(crate::TS_ABANDONED_HB, None),
    //     crate::player::AbandonedChannel(queuectx.clone())
    // );

    Ok(queuectx)
}