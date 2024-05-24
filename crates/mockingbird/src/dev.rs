
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
        Ok(url) => url.to_owned(),
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
    // call.enqueue(track);
    // Ok((track_handle, metadata))
    Ok(())
}
