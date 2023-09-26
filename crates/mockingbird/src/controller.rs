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

use serenity::futures::StreamExt;

use songbird::{
    error::{JoinResult, JoinError},
   TrackEvent
};

use std::{
    process::Stdio,
    time::{Duration, Instant},
    collections::{VecDeque, HashSet},
    sync::Arc,
    collections::HashMap,
};

use crate::player::{MetadataType, QueueContext, TrackRecord, EventEnd};

// use crate::routines::{next_track, join_routine, leave_routine, play_routine};


// #[group]
// #[commands(join, leave, queue, now_playing, skip, clear_seeds, seeds)]
// struct BetterPlayer;


// #[command]
// #[aliases("np", "playing", "now-playing", "playing-now", "nowplaying")]
// #[only_in(guilds)]
// async fn now_playing(ctx: &Context, msg: &Message) -> CommandResult {
//     let guild = msg.guild(&ctx.cache).unwrap();
//     let guild_id = guild.id;
 
//     let manager = songbird::get(ctx)
//         .await
//         .expect("Songbird Voice client placed in at initialisation.")
//         .clone();

//     let call_lock = match manager.get(guild_id) {
//         Some(call) => call,
//         None => {
//             msg.channel_id
//                .say(&ctx.http, "Not in a voice channel to play in")
//                .await?;
//             return Ok(())
//         }
//     };

//     let handle = match call_lock.lock().await.queue().current() {
//         Some(handle) => handle,
//         None => {
//             msg.channel_id
//                .say(&ctx.http, "Nothing is currently playing")
//                .await?;

//             return Ok(())
//         }
//     };
    
//     let reactions = vec![
//         ReactionType::from('\u{23ee}'),
//         ReactionType::from('\u{25c0}'),
//         ReactionType::from('\u{25b6}')  
//     ];
//     let mut glob = ctx.data.write().await; 
//     let qctx = glob.get_mut::<crate::LazyQueueKey>()
//         .expect("Expected LazyQueueKey in TypeMap")
//         .get_mut(&guild_id).unwrap().clone();
        

//     let reply = msg.channel_id.send_message(&ctx.http, |b|
//         b.reactions(reactions.clone())
//          .content(
//             format!(
//                 "{}: {}", qctx.voice_chan_id.mention(),
//                 handle.metadata()
//                     .clone()
//                     .source_url
//                     .unwrap_or("Unknown".to_string())
//             )
//         )).await?;

    
//     let mut collector = reply
//         .await_reactions(&ctx)
//         .collect_limit(10)
//         .timeout(std::time::Duration::from_secs(30))
//         .filter(move |r| {
//            reactions.contains(&r.emoji)
//         }).build();

//     let mut vote : i32 = 0;
//     let mut different_vote : i32= 0;

//     while let Some(reaction) = collector.next().await {
//         let reaction = &reaction.as_ref().as_inner_ref();
//         let emoji = &reaction.emoji;
//         match emoji.as_data().as_str() {
//             "\u{23ee}" => vote += 1,
//             "\u{23c0}" => vote -= 1,
//             "\u{23b6}" =>  different_vote += 1,
//             _ => unreachable!()
//         }
//     }

//     // positive
//     if 0 < vote && vote > different_vote {
//         let cold_queue = qctx.cold_queue.write().await;
//         let first = cold_queue.has_played.front().unwrap();                
//         cold_queue.seeds.push_front(first.metadata.clone());
        
//     }
    
//     else if different_vote > vote || (0 > vote && different_vote > vote.abs())  {
//         let _ = handle.stop();
//     }

//     Ok(())
// }

// #[command]
// #[only_in(guilds)]
// async fn join(ctx: &Context, msg: &Message) -> CommandResult {
//     let connect_to = join_routine(&ctx, msg).await;
    
//     if let Err(ref e) = connect_to {
//         msg.channel_id
//            .say(&ctx.http, format!("Failed to join voice channel: {:?}", e))
//            .await?;
//     };

//     msg.channel_id
//        .say(&ctx.http, format!("Joined {}", connect_to.unwrap().voice_chan_id.mention()))
//        .await?;

//     Ok(())
// }

// #[command]
// #[only_in(guilds)]
// async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
//     let guild = msg.guild(&ctx.cache).unwrap();
//     let guild_id = guild.id;

//     let manager = songbird::get(ctx)
//         .await
//         .expect("songbird voice client placed in at initialisation.")
//         .clone();

//     let handler = manager.get(guild_id);
    
//     if handler.is_none() {
//         msg.reply(ctx, "Not in a voice channel").await?;
//         return Ok(())
//     }
    
//     let handler = handler.unwrap();

//     {
//         let mut call = handler.lock().await;
//         call.remove_all_global_events();
//         call.stop();
//         let _ = call.deafen(false).await;
//     }

//     if let Err(e) = manager.remove(guild_id).await {
//         msg.channel_id
//            .say(&ctx.http, format!("Failed: {:?}", e))
//            .await?;
//     }
    
//     {
//         let mut glob = ctx.data.write().await; 
//         let queue = glob.get_mut::<crate::LazyQueueKey>().expect("Expected LazyQueueKey in TypeMap");
//         queue.remove(&guild_id);
//     }

//     msg.channel_id.say(&ctx.http, "Left voice channel").await?;
//     Ok(())
// }

// #[command]
// #[aliases("play", "p", "q", "add")]
// #[only_in(guilds)]
// async fn queue(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
//     let url = match args.single::<String>() {
//         Ok(url) => url,
//         Err(_) => {
//             msg.channel_id
//                .say(&ctx.http, "Must provide a URL to a video or audio")
//                .await
//                .unwrap();
//             return Ok(());
//         },
//     };

//     if !url.starts_with("http") {
//         msg.channel_id
//            .say(&ctx.http, "Must provide a valid URL")
//            .await
//            .unwrap();
//         return Ok(());
//     };

//     let guild = msg.guild(&ctx.cache).unwrap();
//     let guild_id = guild.id;

//     let manager = songbird::get(ctx)
//         .await
//         .expect("Songbird Voice client placed in at initialisation.")
//         .clone();

//     let qctx: Arc<QueueContext>;

//     let call = match manager.get(guild_id) {
//         Some(call_lock) => {
//             qctx = ctx.data.write().await.get_mut::<LazyQueueKey>().unwrap().get_mut(&guild_id).unwrap().clone();
//             call_lock
//         },
        
//         None => {
//             let tmp = join_routine(ctx, msg).await;            

//             if let Err(ref e) = tmp {
//                 msg.channel_id
//                    .say(&ctx.http, format!("Failed to join voice channel: {:?}", e))
//                    .await
//                    .unwrap();        
//                 return Ok(());
//             };
//             qctx = tmp.unwrap();
//             msg.channel_id
//                    .say(&ctx.http, format!("Joined: {}", qctx.voice_chan_id.mention()))
//                    .await
//                    .unwrap();

//             let call = manager.get(guild_id).ok_or_else(|| JoinError::NoCall);
//             call?
//         }
//     };

//     match Players::from_str(&url)
//         .ok_or_else(|| String::from("Failed to select extractor for URL"))
//     {
//         Ok(player) => {
//             let mut uris = player.fan_collection(url.as_str()).await?;
//             let added = uris.len();
            
//             // YTDLP singles don't work.
//             // so instead, use the original URI.
//             if uris.len() == 1 && player == Players::Ytdl {
//                 uris.clear();
//                 uris.push_back(url.clone());
//             }
            
//             qctx.cold_queue.write().await.queue.extend(uris.drain(..));    

//             let maybe_hot = {
//                 let call = call.lock().await;
//                 call.queue().len() > 0            
//             };

//             drop(call); // probably not needed, but just in case
//             if !maybe_hot {
//                 uqueue_routine(qctx.clone()).await?;
//             }

//             let content = format!(
//                 "Added {} Song(s) [{}] queued",
//                 added,
//                 qctx.cold_queue.read().await.queue.len()
//             );
            
//             msg.channel_id            
//                .say(&ctx.http, &content)
//                .await?;            
//         },

//         Err(_) => {
//             msg.channel_id
//                .say(&ctx.http, format!("Failed to select extractor for URL: {}", url))
//                .await?;
//         }
//     }

//     Ok(())
// }

// #[command]
// #[only_in(guilds)]
// async fn clear_seeds(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
//     let guild = msg.guild(&ctx.cache).unwrap();
//     let guild_id = guild.id;


//     let qctx = ctx.data.write().await
//         .get_mut::<LazyQueueKey>().unwrap()
//         .get_mut(&guild_id).unwrap().clone();

//     {
//         let mut cold_queue = qctx.cold_queue.write().await;
//         cold_queue.seeds.clear();
//         msg.channel_id.say(&ctx.http, String::from("Cleared.")).await?;
//     }
//     Ok(())
// }

// #[command]
// #[only_in(guilds)]
// async fn seeds(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
//     let guild = msg.guild(&ctx.cache).unwrap();
//     let guild_id = guild.id;

//     let qctx = ctx.data.write().await
//         .get_mut::<LazyQueueKey>().unwrap()
//         .get_mut(&guild_id).unwrap().clone();

//     {
//         let cold_queue = qctx.cold_queue.read().await;
//         let seeds = &cold_queue.seeds.range(..)
//             .cloned().collect::<Vec<_>>().join(", ");
//         msg.channel_id.say(&ctx.http, format!("Seeds: {}", seeds)).await?;
//     }
//     Ok(())
// }

// #[command]
// #[only_in(guilds)]
// async fn skip(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
//     let guild = msg.guild(&ctx.cache).unwrap();
//     let guild_id = guild.id;
    
//     let qctx = ctx.data.write().await
//         .get_mut::<LazyQueueKey>().unwrap()
//         .get_mut(&guild_id).unwrap().clone();

//     let cold_queue_len = qctx.cold_queue.read().await.queue.len();
     
//     let skipn = args.remains()
//         .unwrap_or("1")
//         .parse::<isize>()
//         .unwrap_or(1);

//     // stop_event: EventEnd::UnMarked,

//     if 1 > skipn  {
//         msg.channel_id
//            .say(&ctx.http, "Must skip at least 1 song")
//            .await?;
//         return Ok(())
//     }

//     else if skipn >= cold_queue_len as isize + 1 {
//         qctx.cold_queue.write().await.queue.clear();
//     }

//     else {
//         let mut cold_queue = qctx.cold_queue.write().await;
//         let bottom = cold_queue.queue.split_off(skipn as usize - 1);
//         cold_queue.queue.clear();
//         cold_queue.queue.extend(bottom);
//     }
    
//     {
//         let mut cold_queue = qctx.cold_queue.write().await;
//         if let Some(x) = cold_queue.has_played.front_mut()
//         {
//             if let EventEnd::UnMarked = x.stop_event 
//             {
//                 x.stop_event = EventEnd::Skipped;
//                 x.end = Instant::now();
//             }
//         }
//     }

//     let manager = songbird::get(ctx)
//         .await
//         .expect("Songbird Voice client placed in at initialisation.")
//         .clone();

//     match manager.get(guild_id) {
//         Some(call) => {
//             let call = call.lock().await;
//             let queue = call.queue();
//             let _ = queue.skip();
//         }
//         None => {
//             msg.channel_id
//                .say(&ctx.http, "Not in a voice channel to play in")
//                .await?;
//             return Ok(())
//         }
//     };

//     msg.channel_id
//        .say(
//             &ctx.http,
//             format!("Song skipped [{}]: {} in queue.", skipn, cold_queue_len as isize),
//        )
//        .await?;

//     Ok(())
// }