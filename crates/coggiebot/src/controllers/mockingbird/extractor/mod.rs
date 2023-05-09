

use thiserror::Error;
use std::path::PathBuf;
use serenity::client::ClientBuilder; 
use songbird::input::Restartable;

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
#[derive(Debug)]
pub enum PlaySource {
    FileSystem {
        errlog: String,
        ok_paths: Vec<PathBuf>,
    },
    Ytdl {
        uri: String,
    }
}

impl PlaySource
{
    pub async fn to_restartable(self) -> Vec<Restartable> {
        match self {
            PlaySource::FileSystem { errlog, ok_paths } => {
                let mut restartables = Vec::new();
                for path in ok_paths {
                    match Restartable::ffmpeg(path.clone(), true).await
                    {
                        Ok(x) => restartables.push(x.into()),
                        Err(e) => {
                            tracing::error!("failed to create restartable for {}: {}", path.display(), e);
                            tracing::error!("error log: {}", errlog);
                        }
                    }
                }
                restartables
            }
            PlaySource::Ytdl { uri } => {
                let restartable = Restartable::ytdl(uri, true).await.unwrap();
                vec![restartable]
            }
        }
    }
}

#[tracing::instrument]
pub async fn play_source(
    uri: &str,
    #[cfg(feature = "mockingbird-deemix")]
    dx: &deemix::DxConfig
) -> Result<PlaySource, SourceErrors>
{
    const PATH_SZ: usize = 64;

    #[cfg(feature="mockingbird-ytdl")]
    if ytdl::is_ytdl(uri) {
        tracing::info!("streaming with ytdl: {}", uri);
        return Ok(PlaySource::Ytdl { uri: uri.to_owned() });
    }

    let tmpdir = tempfile::tempdir()?;
    let mut src = None;

    tracing::debug!("mkdir: {}", tmpdir.path().display());

    #[cfg(feature="mockingbird-deemix")]
    if deemix::is_deemix(uri) {
        tracing::info!("Downloading deemix from {}", uri);
        src = Some(deemix::deemix(uri, tmpdir.path().to_path_buf(), dx).await?);
    }

    #[cfg(feature="mockingbird-spotify")]
    if deemix::is_spotify(uri) && src.is_none() {
        tracing::info!("using spotify index lookup {}", uri);
        src = Some(deemix::deemix(uri, tmpdir.path().to_path_buf(), dx).await?);
    }

    #[cfg(feature="mockingbird-mp3")]
    if src.is_none() && uri.ends_with(".mp3") {
        if let (resp, true) = mp3::is_mp3(uri).await? {
            tracing::info!("Downloading mp3 from {}", uri);
            src = Some(mp3::download(resp, &tmpdir.path().to_path_buf()).await?);
        }
    }
    
    tracing::info!("src: {:?}", src);
    match src {
        None => return Err(SourceErrors::NoExtractor),
        Some(PlaySource::FileSystem { ref errlog, ref ok_paths }) => 
            return Ok(src.unwrap()),
        _ => unreachable!()
    }    
}


pub async fn init(cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("Mockingbird extractors initialized");
    return deemix::init(cfg).await;
}
