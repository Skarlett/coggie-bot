


////// This is only applied to the first track
// struct DelayRadio(Arc<QueueContext>);
// #[async_trait]
// impl VoiceEventHandler for DelayRadio {
//     async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
//         // let _ = load_queue(self.0.clone()).await;        
//         None
//     }
// }
// async fn play<T: AudioPlayer> (play: T, handler: &mut Call) -> Result<(TrackHandle, Option<MetadataType>), HandlerError>
// {
//     let (input, metadata) = play.load().await?;
//     let (track, track_handle) = create_player(input);
//     handler.enqueue(track);
//     Ok((track_handle, metadata))
// }
// struct TrackEndLoader(Arc<QueueContext>);
// #[async_trait]
// impl VoiceEventHandler for TrackEndLoader {
//     async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
//         if let Some(call) = self.0.manager.get(self.0.guild_id) {
//             let mut call = call.lock().await;
//             let mut cold_queue = self.0.cold_queue.write().await;

//             if call.queue().current().is_none() && cold_queue.queue.is_empty() {
//                 // if user's play list is empty 
                    
//                     // try the preloaded radio track
//                     if let Some(ref mut radio_preload) = cold_queue.radio_next
//                     {          
//                             let preload_result = play_preload(
//                                 &mut call,
//                                 &mut radio_preload.children,
//                                 radio_preload.metadata.clone()
//                                     .map(|x| x.into())
//                             ).await;
                            
//                             match preload_result {
//                                 Err(why) =>{        
//                                     tracing::error!("Failed to play radio track: {}", why);
//                                     return None
//                                 }
//                                 Ok((handle, _)) => {
//                                     handle.add_event(
//                                         Event::Delayed(handle.metadata().duration.unwrap() - TS_PRELOAD_OFFSET),
//                                         PreemptLoader(self.0.clone())
//                                     ).unwrap();

//                                 }
//                             }

//                             cold_queue.radio_next = None;
//                     }
    
//                     drop(call);
//                     drop(cold_queue);
//             }
//             else {

//                 cold_queue.radio_queue.clear();
//                 if let Some(next) = &mut cold_queue.radio_next {
//                     while let Some(mut pid) = next.children.pop() {
//                         let _ = pid.kill();
//                     }
//                 }
//                 cold_queue.radio_next = None;


//                 drop(cold_queue);
//                 drop(call);
//                 let _ = uqueue_routine(self.0.clone()).await;                
//             }
//         }
//         None
//     }
// }

// pub struct AbandonedChannel(Arc<QueueContext>);
// #[async_trait]
// impl VoiceEventHandler for AbandonedChannel {
//     async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
//         let members = self.0.voice_chan_id.members(&self.0.cache).await.unwrap();
//         if members.iter().filter(|x| !x.user.bot).count() > 0 {
//             return None;
//         }

//         leave_routine(
//             self.0.data.clone(),
//             self.0.guild_id.clone(),
//             self.0.manager.clone()
//         ).await.unwrap();

//         Some(Event::Cancel)
//     }
// }

// pub struct PreemptLoader(Arc<QueueContext>);
// #[async_trait]
// impl VoiceEventHandler for PreemptLoader {
//     async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {      
//         // let _ = uqueue_routine(self.0.clone()).await;
//         None
//     }
// }