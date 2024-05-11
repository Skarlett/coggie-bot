use tokio::io::AsyncWriteExt;
use serenity::futures::StreamExt;
use std::path::PathBuf;

use super::PlaySource;

#[derive(Error, Debug)]
pub enum Mp3Error {
    #[error("io error")]
    BadIO(#[from] std::io::Error),

    #[error("could not fetch http response")]
    BadResponse(#[from] reqwest::Error),
}

pub fn human_filesize(n: u64) -> String {
    let base: u64 = 1024;
    let suffixes = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    let i = (n as f64).log(base as f64).floor() as u32;
    let power = base.pow(i);
    let size = n as f64 / power as f64;
    return format!("{}{}", size, suffixes[i as usize]);
}

pub async fn is_mp3(uri: &str) -> Result<(reqwest::Response, bool), Mp3Error> {
    let resp = reqwest::get(uri).await?;
    let headers = resp.headers();
    let content_type = headers.get("Content-Type").unwrap();

    if let Ok("audio/mpeg") = content_type.to_str() {
        return Ok((resp, true));
    }

    tracing::error!("{}: content type is not audio/mpeg", uri);
    return Ok((resp, false));
}

pub async fn download(resp: reqwest::Response, tmpdir: &PathBuf) -> Result<PlaySource, Mp3Error> {
    // Content-Disposition: attachment; filename*=UTF-8''Geostigma.mp3
    let headers = resp.headers();
    let content_disposition = headers.get("Content-Disposition").unwrap();
    let filename = content_disposition.to_str().unwrap().split("filename*=UTF-8''").last().unwrap();
    
    let fp = tmpdir.join(filename);

    tracing::info!("writing: {}", fp.display());
    let mut fd = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&fp)
        .await?;

    let mut stream = resp.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item?;
        fd.write_all(&chunk).await?;
    }

    fd.flush().await?;
    fd.sync_all().await?;

    tracing::info!("wrote: {} [{}]", fp.display(), human_filesize(fd.metadata().await?.len()));

    return Ok(PlaySource::FileSystem { 
        ok_paths: vec![fp],
    });
}
