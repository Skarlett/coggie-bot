use thiserror::Error;
use std::path::PathBuf;
use serenity::client::ClientBuilder; 

#[cfg(feature="mockingbird-deemix")]
mod deemix;

#[cfg(feature="mockingbird-deemix")]
pub use deemix::{DxConfig, DxConfigKey, DxError};

#[cfg(feature="mockingbird-ytdl")]
mod ytdl;

#[cfg(feature="mockingbird-mp3")]
pub use mp3::Mp3Error;

#[cfg(feature="mockingbird-mp3")]
mod mp3;

#[derive(Debug)]
pub enum PlaySource {
    FileSystem {
        ok_paths: Vec<PathBuf>,
    },

    Ytdl {
        uri: String,
    }
}

#[derive(Debug, Error)]
pub enum SourceErrors {
    #[error("no extractor for uri")]
    NoExtractor,

    #[error("failed to create temporary directory")]
    MkTmpFailed(#[from] std::io::Error),

    #[cfg(feature="mockingbird-mp3")]
    #[error("mp3 error")]
    Mp3Error(#[from] mp3::Mp3Error),

    #[cfg(feature="mockingbird-deemix")]
    #[error("deemix error")]
    DeemixError(#[from] deemix::DxError)
}


pub struct PlayRequest<'a> {
    pub uri: &'a str,

    #[cfg(feature = "mockingbird-deemix")]
    pub dx: &'a deemix::DxConfig
}

pub async fn play_source<'a>(
   req: PlayRequest<'a>
) -> Result<PlaySource, SourceErrors>
{
    #[cfg(feature="mockingbird-ytdl")]
    if ytdl::is_ytdl(req.uri) {
        tracing::info!("streaming with ytdl: {}", req.uri);
        return Ok(PlaySource::Ytdl { uri: req.uri.to_owned() });
    }

    let tmpdir = tempfile::tempdir()?;
    let mut src = None;
    let tmp_path = tmpdir.path().to_path_buf();
    tracing::debug!("mkdir: {}", tmp_path.display());

    #[cfg(feature="mockingbird-deemix")]
    if deemix::is_deemix(req.uri) {
        tracing::info!("Downloading deemix from {}", req.uri);
        src = Some(deemix::deemix(req.uri, &tmp_path, &req.dx).await?);
    }

    #[cfg(feature="mockingbird-spotify")]
    if deemix::is_spotify(req.uri) && src.is_none() {
        tracing::info!("using spotify index lookup {}", req.uri);
        src = Some(deemix::deemix(req.uri, &tmp_path, &req.dx).await?);
    }

    #[cfg(feature="mockingbird-mp3")]
    if src.is_none() && req.uri.ends_with(".mp3") {
        if let (resp, true) = mp3::is_mp3(req.uri).await? {
            tracing::info!("Downloading mp3 from {}", req.uri);
            src = Some(mp3::download(resp, &tmp_path).await?);
        }
    }
    
    match src {
        None => return Err(SourceErrors::NoExtractor),
        Some(PlaySource::FileSystem { .. }) => 
            return Ok(src.unwrap()),
        _ => unreachable!()
    }    
}


pub async fn init(mut cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("Mockingbird extractors initialized");
    #[cfg(feature="mockingbird-deemix")]
    { cfg = deemix::init(cfg).await; }
    return cfg
}
