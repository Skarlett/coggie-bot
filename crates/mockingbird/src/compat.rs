use songbird::input::Input;

use std::{
    process::Stdio,
    collections::VecDeque,
    path::PathBuf,
};

use tokio::{
    io::AsyncBufReadExt,
    process::Command,
};

use crate::models::*;

#[cfg(not(feature = "deemix"))]
pub struct FakeMeta(Metadata);

#[cfg(not(feature = "deemix"))]
impl Into<Metadata> for FakeMeta {
    fn into(self) -> Metadata {
        self.0
    }
}

fn process_fan_output(buf: &mut VecDeque<String>, json_buf: Vec<serde_json::Value>, err_cnt: &mut usize, key: &str){
    for x in json_buf {
        if let Some(jmap) = x.as_object() {
            if !jmap.contains_key(key) {
                tracing::error!("{} not found in json", key);
                *err_cnt += 1;
                continue
            }

            buf.push_back(jmap[key].as_str().unwrap().to_owned());
        }
        else {
            tracing::error!("{} not found in json", key);
            *err_cnt += 1;
            continue
        }
    }
    tracing::info!("{} tracks found", buf.len());
}

async fn _urls(cmd: &str, args: &[&str], buf: &mut Vec<serde_json::Value>) -> std::io::Result<()> {
    let child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.wait_with_output().await.unwrap();
    let mut lines = stdout.stdout.lines();

    while let Some(line) = lines.next_line().await? {
        let json =
            serde_json::from_str(&line).unwrap();
        buf.push(json);
    }
    Ok(())
}

/*
 * Some ugly place holders for
 * feature generated code.
*/
#[cfg(feature="deemix")]
pub async fn fan_deezer(uri: &str, buf: &mut VecDeque<String>) -> Result<usize, HandlerError> {
    let mut json_buf = Vec::new();
    let mut err_cnt = 0;
    _urls("deemix-metadata", &[uri], &mut json_buf).await?;

    process_fan_output(buf, json_buf, &mut err_cnt, "link");
    Ok(err_cnt)
}

#[cfg(feature="ytdl")]
pub async fn fan_ytdl(uri: &str, buf: &mut VecDeque<String>) -> Result<usize, HandlerError> {
    let mut json_buf = Vec::new();
    let mut err_cnt = 0;
    _urls("yt-dlp", &["--flat-playlist", "-j", uri], &mut json_buf).await?;

    process_fan_output(buf, json_buf, &mut err_cnt, "url");
    Ok(err_cnt)
}

#[cfg(not(feature="deemix"))]
pub async fn fan_deezer(uri: &str, buf: &mut VecDeque<String>) -> Result<usize, HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(not(feature="ytdl"))]
pub async fn fan_ytdl(_uri: &str, _buf: &mut VecDeque<String>) -> Result<usize, HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(feature = "http-get")]
pub async fn ph_httpget_player(
    uri: &str,
    guild_id: u64,
    ref_fp: &mut PathBuf,
) -> (Result<(Input, Option<MetadataType>), HandlerError>) {
    tracing::info!("[HTTP-GET] Downloading: {}", uri);

    // let fp = tempfile::tempfile()?;
    use rand::Rng;
    let id: String = (0..12)
        .map(|_| char::from(rand::thread_rng().gen_range(97..123)))
        .collect();

    let fp = std::env::temp_dir()
        .join("coggiebot")
        .join(guild_id.to_string());

    match tokio::fs::create_dir_all(&fp).await {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Failed to create temp dir: {}", e);
            return (Err(HandlerError::IOError(e)));
        }
    }
    let fp = fp.join(format!("{}", id));

    match crate::player::get_file(uri, guild_id, &fp).await.map_err(HandlerError::from) {
        Ok(input) => Ok((input, Some(MetadataType::Disk(fp.clone())))),
        Err(e) => {
            if let Ok(true) = tokio::fs::try_exists(&fp).await {
                let _ = tokio::fs::remove_file(&fp).await;
            }
            Err(e)
        }
    }
}

#[cfg(feature = "deemix")]
pub async fn ph_deemix_player(uri: &str) -> Result<(Input, Option<MetadataType>), HandlerError> {
    crate::deemix::deemix(uri).await
        .map_err(HandlerError::from)
        .map(|(input, meta)| (input, meta.map(|x| x.into())))
}

#[cfg(feature = "ytdl")]
pub async fn ph_ytdl_player(uri: &str) -> Result<(Input, Option<MetadataType>), HandlerError> {
    return songbird::ytdl(uri).await.map_err(HandlerError::from)
        .map(|input| (input, None))
}

#[cfg(not(feature = "deemix"))]
pub async fn ph_deemix_player(uri: &str) -> Result<(Input, Option<FakeMeta>), HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(not(feature = "ytdl"))]
pub async fn ph_ytdl_player(_uri: &str) -> Result<(Input, Option<MetadataType>), HandlerError> {
    return Err(HandlerError::NotImplemented)
}

#[cfg(not(feature = "http-get"))]
pub async fn ph_httpget_player(
    _uri: &str,
    _guild_id: u64,
    _ref_fp: &mut PathBuf,
) -> Result<(Input, Option<MetadataType>), HandlerError>
{
    return Err(HandlerError::NotImplemented)
}
