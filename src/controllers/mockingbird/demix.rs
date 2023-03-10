use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::Duration,
};

use std::{
    io::{BufRead, BufReader, Read},
    process::{Command, Stdio},
    default::Default
};
use tokio::{process::Command as TokioCommand, task};

use std::ffi::OsStr;
use serenity::{
    async_trait,
    client::Context,
    framework::{
        standard::{
            macros::{command, group},
            Args,
            CommandResult,
        },
    },
    http::Http,
    model::{channel::Message, prelude::ChannelId},
    prelude::Mentionable,
    Result as SerenityResult,
};

use songbird::{
    input::{
        self,
        restartable::{Restartable, Restart},
        Input,
        Container,
        Codec,
        Metadata,
        error::Error as InputError,
        children_to_reader,
    },
    Event,
    EventContext,
    EventHandler as VoiceEventHandler,
    TrackEvent,
};

#[group]
struct Demix;

/// ARL tokens are used for deezer API access
struct ArlToken;
impl TypeMapKey for ArlToken {
    type Value = String;
}

struct DeezerRestarter<P>
{
    uri: P,
    arl: String
}

struct DeezerConfig
{
    pub arl_token: String,
}


fn deezer(uri: &str, arl: &str, pre_args: &[&str]) -> Result<Input, InputError>
{
    let demix_args = [
        "--arl", arl,
        "-b", "128000",
        uri,
    ];

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

    let mut demix_pipe = Command::new("pipe_demix")
        .args(&demix_args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let taken_stdout = demix_pipe.stdout.take().ok_or(InputError::Stdout)?;
    let ffmpeg = Command::new("ffmpeg")
        .args(pre_args)
        .arg("-i")
        .arg("-")
        .args(&ffmpeg_args)
        .stdin(taken_stdout)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;

    //let metadata = Metadata::from_ytdl_output(value?);

    Ok(Input::new(
        true,
        children_to_reader::<f32>(vec![demix_pipe, ffmpeg]),
        Codec::FloatPcm,
        Container::Raw,
        None,
    ))
}

#[async_trait]
impl Restart for DeezerRestarter<String>
{
    async fn call_restart(&mut self, time: Option<Duration>) -> Result<Input, InputError>
    {
        deezer(&self.uri, &self.arl, &[])
    }
    async fn lazy_init(&mut self) -> Result<(Option<Metadata>, Codec, Container), InputError>
    {
        //deezer(&self.uri, &self.arl, &["-ss", "0:00:05"])
        todo!()
    }
}


#[command("arl")]
async fn get_arl(ctx: &Context, msg: &Message) -> CommandResult {
    let arl = ctx.data.read().await.get::<ArlToken>().expect("Expected CommandCounter in TypeMap.").clone();
    msg.channel_id.say(&ctx.http, arl).await?;
    Ok(())
}


fn deezer_hook() {
    let arl = match ctx.data.read().await.get::<ArlToken>() {
        Some(arl) => arl.clone(),
        None => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, "No ARL token found")
                    .await,
            );

            return Ok(());
        }
    };

    let restarter = match deezer(&url, &arl, &[] ){
        Ok(src) => src,
        Err(e) => {
            check_msg(
                msg.channel_id
                    .say(&ctx.http, format!("Error: {}", e))
                    .await,
            );
            return Ok(());
        }
    };
}



