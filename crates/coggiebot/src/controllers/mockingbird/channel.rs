#[cfg(feature="mockingbird-channel")]
pub async fn on_dj_channel(ctx: &Context, msg: &Message) -> CommandResult {
    let url = match &msg.content
    {
        url if url.starts_with("http") => url.to_string(),
        _ => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "Must provide a URL to a video or audio")
                    .await,
            );

            return Ok(());
        },
    };

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        for x in play_source(&url) {
            match x {
                Ok(src) => handler.enqueue_source(src.into()),
                Err(e) => {
                    tracing::error!("Error Starting source [{:?}]: {:?}", x, e);
                }
            }
            handler.enqueue_source(x.into());
        }

        check_msg(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("Added song to queue: position {}", handler.queue().len()),
                )
                .await,
        );
    }
    else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}
