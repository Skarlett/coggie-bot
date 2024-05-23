#[command]
#[only_in(guilds)]
/// @bot radio [on/off/(default: status)]
async fn crossfade(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
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
                    if qctx.crossfade.load(Ordering::Relaxed)
                    { "on" } else { "off" },
                ).await?; },

        "on" => {
            qctx.crossfade.swap(true, Ordering::Relaxed);
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
