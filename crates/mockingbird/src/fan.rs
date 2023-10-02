use serenity::async_trait;
use crate::player::{MetadataType, TrackRequest};
use crate::ctrlerror::HandlerError;
use crate::deemix::DeemixMetadata;
use songbird::input::Metadata;
use std::collections::VecDeque;
use std::process::Stdio;
use tokio::{process::Command, io::AsyncBufReadExt};

use serenity::model::channel::Message;
use serenity::prelude::Context;


pub struct DeemixUri;
pub struct YtdlUri;


pub trait LinkParser {
    fn parse_url(&mut self, msg: &Message, context: &mut Context) -> Option<TrackRequest>; 
}

#[async_trait]
pub trait FetchMetadata {
    async fn fetch_metadata(self, buf: &mut VecDeque<MetadataType>) -> Result<usize, HandlerError>;
}

impl LinkParser for DeemixUri {
    fn parse_url(&mut self, msg: &Message, context: &mut Context) -> Option<TrackRequest> {
        const DEEMIX: [&'static str; 4] = [
            "deezer.page.link",
            "deezer.com",
            "open.spotify",
            "spotify.link"
        ];

        let uri = msg.content.clone();

        if DEEMIX.iter().any(|x| uri.contains(x)) {
            return Some(TrackRequest::user(uri.to_owned(), msg.author.id))
        }
        None
    }
}

impl LinkParser for YtdlUri {
    fn parse_url(&mut self, msg: &Message, context: &mut Context) -> Option<TrackRequest> {
        const YTDL: [&'static str; 4] = [
            "youtube.com",
            "youtu.be",
            "music.youtube.com",
            "soundcloud.com"
        ];

        let uri = msg.content.clone();

        if YTDL.iter().any(|x|uri.contains(x)) {
            return Some(TrackRequest::user(uri.to_owned(), msg.author.id))
        }
        None
    }
}

#[async_trait]
impl FetchMetadata for DeemixUri {
    async fn fetch_metadata(&mut self, msg: &Message, buf: &mut VecDeque<MetadataType>) -> Result<usize, HandlerError> {
        let mut json_buf = Vec::new();
        let mut err_cnt = 0;
        
        metadata_url("deemix-metadata", &[&self], &mut json_buf).await?;

        for x in json_buf {
            let meta = DeemixMetadata::from_deemix_output(&x);
            buf.push_back(MetadataType::Deemix(meta));
        }

        Ok(err_cnt)
    }   
}

#[async_trait]
impl FetchMetadata for YtdlUri {
    async fn expand(self, buf: &mut VecDeque<MetadataType>) -> Result<usize, HandlerError> {
        let mut json_buf = Vec::new();
        let mut err_cnt = 0;
        metadata_url("yt-dlp", &["--flat-playlist", "-j", &self.0], &mut json_buf).await?;
        
        for x in json_buf {
            let meta = Metadata::from_ytdl_output(x);
            buf.push_back(MetadataType::Standard(meta));
        }

        // process_fan_output(buf, json_buf, &mut err_cnt, "url");
        Ok(err_cnt)
    }
}

async fn metadata_url(cmd: &str, args: &[&str], buf: &mut Vec<serde_json::Value>) -> std::io::Result<()> {
    let child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.wait_with_output().await.unwrap();
    let mut lines = stdout.stdout.lines();
   
    while let Some(line) = lines.next_line().await? {
        let json = serde_json::from_str(&line).unwrap();
        buf.push(json);
    } 
    Ok(())
}