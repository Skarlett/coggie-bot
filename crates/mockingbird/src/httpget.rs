use crate::player::Player;
use tokio::io::AsyncWriteExt;
use serenity::futures::StreamExt;
use std::path::PathBuf;


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


pub async fn from_m3u(uri: &str, tmpdir: &PathBuf) -> Result<PlaySource, Mp3Error> {
    let resp = reqwest::get(uri).await?;
    return get_file(resp, tmpdir).await;
}


pub async fn get_file(resp: reqwest::Response, tmpdir: &PathBuf) -> Result<PlaySource, Mp3Error> {
    async fn unsupported_content(uri: &str, content_disposition: &str) -> Mp3Error {
        tracing::error!("{} [{}]: content type is not supported", uri, content_disposition);
        return Err(Mp3Error::BadResponse(
            reqwest::Error::new(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "content type is not supported"))
        );
    }

    let resp = reqwest::get(uri).await?;
    let headers = resp.headers();
    let content_type = headers.get("Content-Type").unwrap();
    let content_disposition = headers.get("Content-Disposition").unwrap();

    // Content-Disposition: attachment; filename*=UTF-8''Geostigma.mp3
    let filename = content_disposition.to_str().unwrap().split("filename*=UTF-8''").last().unwrap();
    let fp = tmpdir.join(filename);

    match content_type.to_str() {
        Ok(x) => {
            if re::Regex::new(r"^audio/[mp3|flac|wav]$").unwrap().is_match(x) {

                tracing::info!("{}: content type is audio/mpeg", uri);
                let content_disposition = headers.get("Content-Disposition").unwrap();

                // Content-Disposition: attachment; filename*=UTF-8''Geostigma.mp3
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

                input::ffmpeg(&fp).await
            }
            else
                { return unsupported_content(uri, content_disposition).await; }
        }
        Err(_) =>
            { return unsupported_content(uri, content_disposition).await; }
    }
}
