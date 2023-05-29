use super::extractor::{DxConfigKey, DxConfig};
use songbird::{
    constants::SAMPLE_RATE_RAW,
    input::{
        children_to_reader,
        error::{Error as SongbirdError, Result as SongbirdResult},
        Codec,
        Container,
        Metadata,
        Input,
        restartable::Restart
    },
    create_player
};
use serenity::{
    model::prelude::*,
    prelude::*,
    framework::standard::{
        macros::{command, group},
        CommandResult, Args,
    },
};
use std::{
    io::{BufReader, BufRead, Read},
    process::Stdio,
    time::Duration
};
use serde_json::Value;
#[group]
#[commands(queue)]
pub struct Beta;

#[command("deezer")]
#[aliases("dx")]
#[only_in(guilds)]
async fn queue(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
                msg.channel_id
                    .say(&ctx.http, "Must provide a URL to a video or audio")
                    .await;
            return Ok(());
        },
    };

    if !url.starts_with("http") {
        msg.channel_id
            .say(&ctx.http, "Must provide a valid URL")
            .await;
        return Ok(());
    }

    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;        

        match deemix(&url).await {
            Ok(input) => {
                let (track, _track_handle) = create_player(input);
                handler.enqueue(track);
            }
            Err(e) => { msg.reply(&ctx.http, format!("Error: {}", e)).await.unwrap(); }
        }

        msg.channel_id
            .say(
                &ctx.http,
                format!("Added song to queue: position {}", handler.queue().len()),
            )
            .await;
    }
    else {
            msg.channel_id
                .say(&ctx.http, "Not in a voice channel to play in")
                .await;
    }

    Ok(())
}
struct DeemixRestarter<P> {
    uri: P,
}

#[serenity::async_trait]
impl<P> Restart for DeemixRestarter<P>
where
    P: AsRef<str> + Send + Sync,
{
    async fn call_restart(&mut self, time: Option<Duration>) -> Result<Input, SongbirdError> {
        if let Some(time) = time {
            let ts = format!("{:.3}", time.as_secs_f64());
            _deemix(self.uri.as_ref(), &["-ss", &ts]).await
        } else {
            deemix(self.uri.as_ref()).await
        }
    }

    async fn lazy_init(&mut self) -> Result<(Option<Metadata>, Codec, Container), SongbirdError> {
        Ok(( Some(Metadata { duration: Some(std::time::Duration::from_secs(200)), ..Default::default()}), Codec::FloatPcm, Container::Raw))
    }
}

pub async fn deemix(
    uri: &str,
) -> SongbirdResult<Input>{ 
    _deemix(uri, &[]).await
}

// #[tracing::instrument]
pub async fn _deemix(
    uri: &str,
    pre_args: &[&str],
) -> SongbirdResult<Input>
{
    let ffmpeg_args = [
        "-f",
        "s16le",
        "-ac",
        "2",
        "-ar",
        "48000",
        "-acodec",
        "pcm_f32le",
        "-",
    ];
    
    tracing::info!("Running: deemix-stream {} {}", pre_args.join(" "), uri);

    let mut deemix = std::process::Command::new("deemix-stream")
        .arg(uri.trim())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    let stderr = deemix.stderr.take();
    let (_returned_stderr, value) = tokio::task::spawn_blocking(move || {
        let mut s = stderr.unwrap();
        let out: SongbirdResult<Value> = {
            let mut o_vec = vec![];
            let mut serde_read = BufReader::new(s.by_ref());
            // Newline...
            if let Ok(len) = serde_read.read_until(0xA, &mut o_vec) {
                serde_json::from_slice(&o_vec[..len]).map_err(|err| SongbirdError::Json {
                    error: { 
                        tracing::error!("TRIED PARSING \n {}", String::from_utf8_lossy(&o_vec));
                        err
                    },
                    parsed_text: std::str::from_utf8(&o_vec).unwrap_or_default().to_string(),
                })
            } else {
                tracing::error!("TRIED PARSING \n {}", String::from_utf8_lossy(&o_vec));
                SongbirdResult::Err(SongbirdError::Metadata)
            }
        };

        (s, out)
    })
    .await
    .map_err(|_| SongbirdError::Metadata)?;

    deemix.stderr = Some(_returned_stderr);
    
    let taken_stdout = deemix.stdout.take().ok_or(SongbirdError::Stdout)?;

    tracing::info!("running ffmpeg");
    let ffmpeg = std::process::Command::new("ffmpeg")
        .args(pre_args)
        .arg("-i")
        .arg("-")
        .args(&ffmpeg_args)
        .stdin(taken_stdout)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start child process");
    let metadata = Some(metadata_from_deemix_output(value?));

    tracing::info!("deezer metadata {:?}", metadata);

    // Wait for ffmpeg to read stream
    tokio::time::sleep(std::time::Duration::from_secs_f64(2.5)).await;
    
    Ok(Input::new(
        true,
        children_to_reader::<f32>(vec![deemix, ffmpeg]),
        Codec::FloatPcm,
        Container::Raw,
        metadata,
    ))
}

fn metadata_from_deemix_output(val: serde_json::Value) -> Metadata
{
    let obj = val.as_object();

    let track = obj
        .and_then(|m| m.get("title"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let artist = obj
        .and_then(|m| m.get("artist"))
        .and_then(|x| x.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string);
 
   let duration = obj
        .and_then(|m| m.get("duration"))
        .and_then(Value::as_f64)
        .map(Duration::from_secs_f64);
    
    Metadata {
        track,
        artist,
        channels: Some(2),
        duration,
        sample_rate: Some(SAMPLE_RATE_RAW as u32),
        ..Default::default()
    }
}