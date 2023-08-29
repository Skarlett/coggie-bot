use tokio::fs::File;
use serenity::{framework::standard::{
    macros::{command, group},
    CommandResult, Args,
}, http::Http};

use serenity::model::channel::Message;
use serenity::prelude::*;

use songbird::Call;
use songbird::input::{Input, error::Error as SongbirdError};
use songbird::create_player;

#[group]
#[commands(
    deafen, join, leave, mute, skip, stop, undeafen, unmute, queue, arl_check, arl_raw
)]
struct Commands;

enum Players {
    Ytdl,
    Deemix,
}

fn warn_unimplemented() -> &'static str {
    "This feature is not implemented."
}

#[allow(unused_variables)]
enum HandlerError {
    Songbird(SongbirdError),
    NotImplemented,
}

impl From<SongbirdError> for HandlerError {
    fn from(err: SongbirdError) -> Self {
        HandlerError::Songbird(err)
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Songbird(err) => write!(f, "Songbird error: {}", err),
            Self::NotImplemented => write!(f, "This feature is not implemented."),
        }
    }
}

/*
 * Some ugly place holders for
 * feature generated code.
*/
#[cfg(feature = "deemix")]
async fn ph_deemix_player(uri: &str) -> Result<Input, HandlerError>
{ crate::deemix::deemix(uri).await.map_err(HandlerError::from) }

#[cfg(not(feature = "deemix"))]
async fn ph_deemix_player(uri: &str) -> Result<Input, HandlerError>
{ return Err(HandlerError::NotImplemented) }

#[cfg(feature = "ytdl")]
async fn ph_ytdl_player(uri: &str) -> Result<Input, HandlerError>
{ return songbird::ytdl(uri).await.map_err(HandlerError::from)  }

#[cfg(not(feature = "ytdl"))]
async fn ph_ytdl_player(uri: &str) -> Result<Input, HandlerError>
{ return Err(HandlerError::NotImplemented) }


impl Players {
    fn from_str(data : &str) -> Option<Self>
    {
        const DEEMIX: [&'static str; 3] = ["deezer.page.link", "deezer.com", "open.spotify"];
        const YTDL: [&'static str; 4] = ["youtube.com", "youtu.be", "music.youtube.com", "soundcloud.com"];
    
        if DEEMIX.iter().any(|x|data.contains(x)) { return Some(Self::Deemix) }
        else if YTDL.iter().any(|x|data.contains(x)) {return Some(Self::Ytdl) }
        else { return None }
    }

    async fn play(&self, ctx: &Http, msg: &Message, handler: &mut Call,  uri: &str) -> Result<(), SongbirdError>
    {
        let input = match self {
            Self::Deemix => ph_deemix_player(uri).await,
            Self::Ytdl => ph_ytdl_player(uri).await
        };

        match input {
            Ok(input) => {
                let (track, _track_handle) = create_player(input);
                handler.enqueue(track);
            }

            Err(HandlerError::NotImplemented) => {
                msg.channel_id.say(&ctx, warn_unimplemented()).await;
                return Ok(())
            }

            Err(HandlerError::Songbird(err)) => return Err(err),
        }

        Ok(())
    }
}

#[command]
#[aliases("play")]
#[only_in(guilds)]
async fn queue(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "Must provide a URL to a video or audio")
                    .await,
            );

            return Ok(());
        },
    };

    if !url.starts_with("http") {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Must provide a valid URL")
                .await,
        );

        return Ok(());
    }

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let http = ctx.http.clone();

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();


    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;

        let player = Players::from_str(&url)
            .ok_or_else(|| String::from("Failed to select extractor for URL"));
        
        match player {
            Ok(player) => player.play(&http, msg, &mut handler, &url).await?,
            Err(e) => {
                check_msg(
                    msg.channel_id
                        .say(
                            &ctx.http,
                            format!("Error: {}", e),
                        ).await
                );
                return Ok(());
            }

        }

        check_msg(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("Added song to queue: position {}", handler.queue().len()),
                )
                .await,
        );
        Ok(())
    }

    else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
        Ok(())
    }
}

#[command]
async fn deafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    let mut handler = handler_lock.lock().await;
    if handler.is_deaf() {
        check_msg(msg.channel_id.say(&ctx.http, "Already deafened").await);
    } else {
        if let Err(e) = handler.deafen(true).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Deafened").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let channel_id = guild
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let (_handle_lock, success) = manager.join(guild_id, connect_to).await;

    let reply = match success {
        Ok(_) => format!("Joined {}", connect_to.mention()),
        Err(e) => format!("Failed to join voice channel: {:?}", e),
    };

    check_msg(
        msg.channel_id
           .say(&ctx.http, reply)
           .await
    );

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        if let Err(e) = manager.remove(guild_id).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn mute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let handler_lock = match manager.get(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    let mut handler = handler_lock.lock().await;

    if handler.is_mute() {
        check_msg(msg.channel_id.say(&ctx.http, "Already muted").await);
    } else {
        if let Err(e) = handler.mute(true).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Now muted").await);
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn skip(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.skip();

        check_msg(
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("Song skipped: {} in queue.",
                        queue.len()
                    ),
                )
                .await,
        );
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn stop(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let _ = queue.stop();
        check_msg(msg.channel_id.say(&ctx.http, "Queue cleared.").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn undeafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.deafen(false).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Undeafened").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to undeafen in")
                .await,
        );
    }

    Ok(())
}

#[command]
#[only_in(guilds)]
async fn unmute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        if let Err(e) = handler.mute(false).await {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Failed: {:?}", e))
                    .await,
            );
        }

        check_msg(msg.channel_id.say(&ctx.http, "Unmuted").await);
    } else {
        check_msg(
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to unmute in")
                .await,
        );
    }

    Ok(())
}

fn santitize_arl(arl: &str) -> Result<(), ()>
{
    if arl.trim().len() == 192 && arl.chars().all(|c| c.is_ascii_hexdigit())
    { return Ok(()) }
    Err(())
}

#[command("arl-raw")]
#[cfg(feature = "check")]
async fn arl_raw(
    ctx: &Context,
    msg: &Message,
    mut args: Args
) -> CommandResult
{
    let mut iargs = args.iter::<String>();
    while let Some(Ok(arl)) = iargs.next() {
        if let Err(()) = santitize_arl(&arl) {
            check_msg(msg.channel_id.say(&ctx.http, "Invalid ARL").await);
            continue;
        }

        let check = crate::check::get_arl_data(&arl).await?;

        check_msg(msg.channel_id.send_files(
            &ctx.http,
            vec![
                (serde_json::to_string_pretty(&check)?.as_bytes(), format!("{}.json", arl).as_str())
            ],
            |m| m
        ).await);
    }
    Ok(())
}

#[command("arl")]
#[cfg(feature = "check")]
async fn arl_check(
    ctx: &Context,
    msg: &Message,
    mut args: Args
) -> CommandResult
{
    use chrono::prelude::*;

    const BLANKSPACE: &'static str = "\x20"; // 0x20
    const RED: u32    = 0x00FF0000;
    const GREEN: u32  = 0x0000FF00;
    const YELLOW: u32 = 0x00FFFF00;

    let mut iargs = args.iter::<String>();

    while let Some(Ok(arl)) = dbg!(iargs.next())
    {
        if let Err(()) = santitize_arl(&arl) {
            check_msg(msg.channel_id.say(&ctx.http, "Invalid ARL").await);
            continue;
        }

        let arl = arl.trim();
        if dbg!(arl.len()) != 192 {
            check_msg(
            msg.channel_id
                    .say(&ctx.http, "Must provide an ARL")
                    .await,
            );
            continue
        }

        let check = match crate::check::check_arl(&arl).await {
            Ok(check) => check,
            Err(e) => {
                check_msg(
                    msg.channel_id
                        .say(&ctx.http, format!("Error: {}", e))
                        .await,
                );
                continue
            }
        };

        let is_usa = check.country.to_ascii_lowercase() == "us";

        let color = if check.dank()
          { GREEN }
        else if check.lossless() && check.explicit()
          { YELLOW }
        else
          { RED };

        fn checkmark(check: bool) -> &'static str {
            if check { ":white_check_mark:" }
            else { ":x:" }
        }

        let explicit = checkmark(check.explicit());
        let lossless = checkmark(check.lossless());
        let country_checkmark = checkmark(is_usa);
        let sound_quality_table = check.tabulize().await.expect("Failed to collect column -t");


        let naive = NaiveDateTime::from_timestamp_opt(check.expiration, 0).unwrap();
        let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
        let expiredate = datetime.format("%Y-%m-%d %H:%M:%S");
        let expire_checkmark = checkmark(datetime > Utc::now());

        check_msg(msg.channel_id
           .send_message(&ctx.http, |m|
                {
                  let m = 
                    m.add_embed(|e| 
                        e.title("ARL Check")
                            .color(color)
                            .description(&arl)
                            .fields(vec![
                                (format!("Allows Explicit: {explicit}").as_str(), BLANKSPACE, false),
                                (format!("Allows Lossless: {lossless}").as_str(), BLANKSPACE, false),
                                (format!("Country: {} {}", check.country, country_checkmark).as_str(), BLANKSPACE, false),
                                (format!("Inscription date: {}", check.inscription).as_str(), BLANKSPACE, false),
                                (format!("Expiration: {expiredate} {expire_checkmark}").as_str(), BLANKSPACE, false),
                                (format!("Email: {}", check.email).as_str(), BLANKSPACE, false),
                                (format!("Offer {} ({})", check.offer_name, check.offer_id).as_str(), BLANKSPACE, false),
                                (BLANKSPACE, &format!("```\n{}\n```", sound_quality_table), false),
                            ])
                            .footer(|f| f.text(format!("**Deezer uses Mobile API.**")))
                        );
                    m
    }).await);
            
    }

    Ok(())
}

use serenity::Result as SerenityResult;
/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        tracing::error!("Error sending message: {:?}", why);
    }
}
