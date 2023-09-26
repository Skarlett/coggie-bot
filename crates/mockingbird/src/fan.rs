use serenity::async_trait;
use crate::player::MetadataType;
use crate::ctrlerror::HandlerError;
use crate::deemix::DeemixMetadata;
use songbird::input::Metadata;
use std::collections::VecDeque;
use std::process::Stdio;
use tokio::{process::Command, io::AsyncBufReadExt};


pub struct DeemixUri(pub String);
pub struct YtdlUri(pub String);
pub struct DeemixParser;


pub trait LinkParser<T> {
    fn parse_url(uri: &str) -> Option<T>; 
}

#[async_trait]
pub trait FanUri {
    async fn fan(self, buf: &mut VecDeque<MetadataType>) -> Result<usize, HandlerError>;
}

impl LinkParser<DeemixUri> for DeemixParser {
    fn parse_url(uri: &str) -> Option<DeemixUri> {
        const DEEMIX: [&'static str; 4] = [
            "deezer.page.link",
            "deezer.com",
            "open.spotify",
            "spotify.link"
        ];

        if DEEMIX.iter().any(|x| uri.contains(x)) {
            return Some(DeemixUri(uri.to_owned()))
        }
        None
    }
}

struct YtdlParser;
impl LinkParser<YtdlUri> for YtdlParser {
    fn parse_url(uri: &str) -> Option<YtdlUri> {
        const YTDL: [&'static str; 4] = [
            "youtube.com",
            "youtu.be",
            "music.youtube.com",
            "soundcloud.com"
        ];
        if YTDL.iter().any(|x|uri.contains(x)) {
            return Some(YtdlUri(uri.to_owned()))
        }
        None
    }
}

#[async_trait]
impl FanUri for DeemixUri {
    async fn fan(self, buf: &mut VecDeque<MetadataType>) -> Result<usize, HandlerError> {
        let mut json_buf = Vec::new();
        let mut err_cnt = 0;
        
        metadata_url("deemix-metadata", &[&self.0], &mut json_buf).await?;

        for x in json_buf {
            let meta = DeemixMetadata::from_deemix_output(&x);
            buf.push_back(MetadataType::Deemix(meta));
        }

        Ok(err_cnt)
    }   
}

#[async_trait]
impl FanUri for YtdlUri {
    async fn fan(self, buf: &mut VecDeque<MetadataType>) -> Result<usize, HandlerError> {
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