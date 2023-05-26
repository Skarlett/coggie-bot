use std::env;
use crate::controllers::mockingbird::extractor::PlaySource;
use std::path::{Path, PathBuf};
use serenity::prelude::TypeMapKey;
use async_walkdir::WalkDir;
use tokio::io::AsyncWriteExt;
use std::process::Stdio;
use serenity::{
    futures::StreamExt,
    client::ClientBuilder
};

#[cfg(feature="mockingbird-spotify")]
use serde::Serialize;

pub struct DxConfigKey;
impl TypeMapKey for DxConfigKey {
    type Value = DxConfig;
}

#[derive(thiserror::Error, Debug)]
pub enum DxError {
    #[error("io error")]
    BadIO(#[from] std::io::Error),

    #[error("could not parse track sequence")]
    TrackParseError(#[from] std::num::ParseIntError),

    #[error("environment missing ARL variable")]
    MissingARL,
}

fn track_number(name: &str) -> Result<u32, std::num::ParseIntError> {
    name.split(" - ").collect::<Vec<&str>>().get(0).unwrap().parse::<u32>()
}

pub fn is_deemix(uri: &str) -> bool {
    ["deezer.com", "deezer.page.link"]
        .iter()
        .any(|x| uri.contains(x))
}

#[allow(unused)]
pub fn is_spotify(uri: &str) -> bool {
    ["spotify.com", "open.spotify"]
        .iter()
        .any(|x| uri.contains(x))
}

pub async fn init(cfg: ClientBuilder) -> ClientBuilder {
    #[allow(unused_mut)]
    let mut dx = DxConfig::new(
        env::var("DEEMIX_ARL").ok()      
    );

    tracing::debug!("INIT DEEMIX-CONFIG: {:?}", dx);

    if dx.arl.is_none() {
        tracing::error!("deemix based services will not be available: ds incomplete:  {:?}", dx);
        return cfg.type_map_insert::<DxConfigKey>(DxConfig::new(None))
    }

    tracing::info!("deemix credentials found, enabling support");

    #[cfg(feature="mockingbird-spotify")]
    let dx = {
        let id = env::var("SPOTIFY_CLIENT_ID");
        let key = env::var("SPOTIFY_CLIENT_SECRET");

        match (id, key) {
            (Ok(id), Ok(key)) => {
                tracing::info!("Spotify credentials found, enabling spotify support {} {}", id, key);
                dx.spotify = Some(DxSpotifyCfg::new(id, key));
                dx
            },
            _ => {
                tracing::error!("SPOTIFY_CLIENT_ID or SPOTIFY_CLIENT_SECRET is not set, spotify based services will not be available");
                dx
            }
        }
    };

    tracing::info!("Initializing deemix cache");
    tracing::info!("deemix: {:?}", &dx);
    dx.init_cache().await.unwrap();

    tracing::info!("mockingbird-deemix Initialized");

    cfg.type_map_insert::<DxConfigKey>(dx)
}

// #[tracing::instrument]
pub async fn deemix(
    uri: &str,
    dldir: &PathBuf,
    dx: &DxConfig
) -> Result<PlaySource, DxError>
{
    tracing::info!("RUNNING: deemix --portable -p {} {}", dldir.display(), uri);
    let child = tokio::process::Command::new("deemix")
        .env("REQUESTS_CA_BUNDLE", "")
        .env("CURL_CA_BUNDLE", "")
        .env("HOME", dx.cache.as_ref())
        .current_dir(dx.cache.as_ref().join(".config/deemix"))
        .arg("--portable")
        .arg("-p").arg(&dldir)
        .arg(uri)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start child process");

    let out = child.wait_with_output().await?;
    
    tracing::info!("deemix exit code: {}", out.status);
    tracing::warn!("deemix stderr: {}", String::from_utf8_lossy(&out.stderr[..]));
    tracing::debug!("deemix stdout: {}", String::from_utf8_lossy(&out.stdout[..]));
    
    let paths = process_dir(&dldir, &dx.cache.as_ref().join("music")).await?;
    // tokio::fs::remove_dir_all(&tmpdir).await?;
    
    return Ok(PlaySource::FileSystem {
        ok_paths: paths,
    });
}

#[derive(Debug)]
pub struct DxConfig {
    arl: Option<String>,
    
    pub cache: tempfile::TempDir,

    #[cfg(feature="mockingbird-spotify")]
    spotify: Option<DxSpotifyCfg>,
}

impl DxConfig {
    pub fn new(arl: Option<String>) -> Self {
        Self {
            arl,
            cache: tempfile::tempdir().unwrap(),
            #[cfg(feature="mockingbird-spotify")]
            spotify: None,
        }
    }
 
    pub async fn init_cache(&self) -> Result<(), DxError> {
        if !self.cache.path().exists() {
            tracing::info!("creating cache directory: {:?}", self.cache.path().display());
            tokio::fs::create_dir_all(&self.cache.path()).await?;
        }
        else {
            tracing::info!("cache directory exists {:?}", self.cache.path().display());
        }
        let dotconfig = self.cache.path().join(".config/deemix");
        tokio::fs::create_dir_all(&dotconfig).await?;

        let test = self.cache.path().join("test.json");
        let action = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&test)
            .await?
            .write_all(b"{}")
            .await;

        tokio::fs::remove_file(&test).await?;
        if let Err(why) = action {
            tracing::error!("could not write to cache directory: {}", why);
            panic!("bad cache directory")
        } 

        workspace(&self, &dotconfig).await?;
        Ok(())        
    }

}

async fn workspace(dx: &DxConfig, at: &Path) -> Result<(), DxError> {
    let conf_data = include_str!("deemix.json");

    if dx.arl.is_none() {
        tracing::error!("ARL is none {:?}", dx.arl);
        return Err(DxError::MissingARL)
    }

    let root = at;
    let pbank = root.join("music");
    let fconfig = root.join("config.json");

    tracing::info!("Creating deemix workspace: {}", root.display());

    if ! pbank.exists() {
        tokio::fs::create_dir(&pbank).await?;
    }

   tracing::info!("Creating deemix config: {}", fconfig.display());
   tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(fconfig)
        .await?
        .write_all(conf_data.as_bytes())
        .await?;

    let farl = root.join(".arl");

    tracing::info!("Creating deemix arl: {}", farl.display());
    tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(farl)
        .await?
        .write_all(dx.arl.as_ref().unwrap().as_bytes())
        .await?;

    #[cfg(feature="mockingbird-spotify")]
    if let Some(ref spot_cfg) = dx.spotify {
        spotify_workspace(spot_cfg, &pconfig).await?;
    }

    return Ok(())
}

//#[tracing::instrument]
async fn process_dir(tmpdir: &Path, pbank: &Path) -> Result<Vec<PathBuf>, DxError>
{
    tracing::info!("Processing deemix output directory: {}", tmpdir.display());
    let mut entries = WalkDir::new(tmpdir);
    let mut data: Vec<(u32, async_walkdir::DirEntry)> = Vec::new();
    let mut ret = Vec::new();
    let mut match_tn: u32 = 1;

    while let Some(x) = entries.next().await {
        match x {
            Ok(entry) => if entry.metadata().await?.is_file() {
                tracing::info!("Found file: {}", entry.file_name().to_str().unwrap());
                let track_num = track_number(
                    entry.file_name().to_str().unwrap()
                );

                let tn = match track_num {
                    Ok(tn) => {
                        if match_tn != tn {
                            match_tn = tn;
                        }
                        tn
                    }
                    Err(_) => {
                        tracing::warn!("Failed to parse track number. Assuming value {}", match_tn);
                        match_tn
                    }
                };
                
                match_tn += 1;
                data.push((tn, entry));
            },
            Err(e) => tracing::error!("Error: {}", e)
        }
    }

    data.sort_by(|(n1, _), (n2, _)| {
        n1.cmp(&n2)
    });

    for (n, entry) in data {
        if entry.metadata().await?.is_file() {
            let new_path = pbank.join(entry.file_name());    
            
            tracing::info!("Queueing: {}: {}", n, entry.path().display());
            
            tokio::fs::rename(entry.path(), &new_path).await?;    
            ret.push(new_path)
        }
    }

    tracing::info!("moving deemix output to: {}", pbank.display());
    Ok(ret)
}


#[cfg(feature="mockingbird-spotify")]
#[derive(Serialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct DxSpotifyCfg {
    clientId: String,
    clientSecret: String,
    fallbackSearch: bool,
}

#[cfg(feature="mockingbird-spotify")]
impl DxSpotifyCfg {
    #[allow(non_snake_case)]
    pub fn new(clientId: String, clientSecret: String) -> Self {
        Self {
            clientId,
            clientSecret,
            fallbackSearch: false,
        }
    }
}

#[cfg(feature="mockingbird-spotify")]
#[tracing::instrument]
pub async fn spotify_workspace(spotify: &DxSpotifyCfg, pconfig: &PathBuf) -> std::io::Result<()> {
    let pspot = pconfig.join("spotify");
    let config = pspot.join("config.json");

    tracing::debug!("Generating spotify config dir {}", pspot.display());

    if !pspot.exists() {
        tokio::fs::create_dir(pspot).await?;
    }

    let spotify = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&config)
        .await?
        .write_all(serde_json::to_string(spotify).unwrap().as_bytes())
        .await?;

    tracing::debug!("wrote config {}", config.display());
    Ok(spotify)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::var;
    use std::path::PathBuf;

    #[test]
    #[cfg(feature="mockingbird-deemix")]
    fn path_deemix() {
        let paths = var("PATH").unwrap();
        assert!(paths.split(':').filter(|p| PathBuf::from(p).join("deemix").exists()).count() == 1);
    }

    #[test]
    fn test_deemix() {
        let uri = "https://open.spotify.com/track/2YpeDb67231RjR0MgVLzsG?si=8e9e9e9e9e9e9e9e";
        assert!(is_spotify(uri));
    }
}