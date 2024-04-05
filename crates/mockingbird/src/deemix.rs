use std::io::{BufReader, BufRead, Read};
use songbird::{
    constants::SAMPLE_RATE_RAW,
    input::{
        children_to_reader,
        error::Error as SongbirdError,
        Codec,
        Container,
        Metadata,
        Input,
        restartable::Restart
    },
};
use std::{
    process::Stdio,
    time::Duration
};
use serde_json::Value;
use std::os::fd::AsRawFd;
use tokio::io::AsyncReadExt;
use cutils::{availbytes, bigpipe, max_pipe_size, PipeError};

#[derive(Debug)]
pub enum DeemixError {
    BadJson(String),
    Metadata,
    IO(std::io::Error),
    ParseInt(core::num::ParseIntError),
    Songbird(SongbirdError),
    Tokio(tokio::task::JoinError),
}

impl Into<SongbirdError> for DeemixError {
    fn into(self) -> SongbirdError {
        match self {
            DeemixError::BadJson(_) 
            | DeemixError::ParseInt(_)
            | DeemixError::Metadata 
            => SongbirdError::Metadata,
            
            DeemixError::IO(e) => SongbirdError::Io(e),
            DeemixError::Songbird(e) => e,
            DeemixError::Tokio(e) 
            => SongbirdError::Io(
                std::io::Error::new(std::io::ErrorKind::Other, e)
            ),
        }
    }
}

impl std::fmt::Display for DeemixError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DeemixError::BadJson(s) => write!(f, "Bad JSON: {}", s),
            DeemixError::Metadata => write!(f, "Metadata error"),
            DeemixError::IO(e) => write!(f, "Process error: {}", e),
            DeemixError::ParseInt(e) => write!(f, "Parse int error: {}", e),
            DeemixError::Songbird(e) => write!(f, "Songbird error: {}", e),
            DeemixError::Tokio(e) => write!(f, "Tokio error: {}", e),
        }
    }
}

impl From<SongbirdError> for DeemixError {
    fn from(e: SongbirdError) -> Self {
        DeemixError::Songbird(e)
    }
}

impl From<std::io::Error> for DeemixError {
    fn from(e: std::io::Error) -> Self {
        DeemixError::IO(e)
    }
}

impl From<tokio::task::JoinError> for DeemixError {
    fn from(e: tokio::task::JoinError) -> Self {
        DeemixError::Tokio(e)
    }
}

impl From<core::num::ParseIntError> for DeemixError {
    fn from(e: core::num::ParseIntError) -> Self {
        DeemixError::ParseInt(e)
    }
}

impl std::error::Error for DeemixError {}

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
            _deemix(self.uri.as_ref(), &["-ss", &ts])
                .await
                .map_err(DeemixError::into)
        } else {
            deemix(self.uri.as_ref())
                .await
                .map_err(DeemixError::into)
        }
    }

    async fn lazy_init(&mut self) -> Result<(Option<Metadata>, Codec, Container), SongbirdError> {
        Ok(( Some(deemix_metadata(self.uri.as_ref()).await.unwrap()), Codec::FloatPcm, Container::Raw))
    }
}

pub async fn deemix_metadata(uri: &str) -> std::io::Result<Metadata> {
    let deemix = tokio::process::Command::new("deemix-metadata")
        .arg(uri.trim())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let output = deemix.wait_with_output().await?;
    
    Ok(metadata_from_deemix_output(&serde_json::from_slice(&output.stdout[..])?))
}

fn process_stderr(s: &mut std::process::ChildStderr) -> Result<Value, DeemixError> {
    let mut o_vec = vec![];
    let mut reader = BufReader::new(s.by_ref());

    // read until new line
    reader.read_until(0xA, &mut o_vec)
        .map_err(|_| DeemixError::Metadata)?;

    match serde_json::from_slice::<Value>(&o_vec.as_slice()) {
        Ok(json) => Ok(json),        
        Err(_) => {
            let mut buf: [u8; 2048] = [0; 2048];
            // If process crashes
            // BufReader::read_to_end will hang
            // until EOF is encountered (Never)
            // reader.read_to_end(&mut o_vec).unwrap();
            // -- so instead, use fixed size buffer
            while let Ok(n) = reader.read(&mut buf) {
                if n > 0 {
                    o_vec.extend_from_slice(&buf[..n]);
                    continue;
                }
                else { break; }
            }

            let text = String::from_utf8_lossy(&o_vec);
            return Err(DeemixError::BadJson(text.to_string()));
        }
    }
}


pub async fn deemix(
    uri: &str,
) -> Result<Input, DeemixError> {
    _deemix(uri, &[])
        .await
}

pub async fn _deemix(
    uri: &str,
    pre_args: &[&str],
) -> Result<Input, DeemixError>
{
    let pipesize = max_pipe_size().await.unwrap();
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
        .arg("-hq")
        .arg("1")
        .arg(uri.trim())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    let deemix_out = deemix.stdout.as_ref().unwrap().as_raw_fd();
    unsafe { bigpipe(deemix_out, pipesize); }

    let stderr = deemix.stderr.take();
    // Read first line of stderr
    // for metadata, but read entire buffer if error.
    let threadout = tokio::task::spawn_blocking(move || {
        let mut s = stderr.unwrap();        
        let out = process_stderr(&mut s);  
        (s, out)
    })
    .await?;

    let (returned_stderr, value) = threadout;

    deemix.stderr = Some(returned_stderr);
    
    let metadata_raw = value?;
    if let Some(_) = metadata_raw.get("error") {
        return Err(DeemixError::Metadata);
    }

    let _filesize = metadata_raw["filesize"].as_u64();
    let metadata = Some(metadata_from_deemix_output(&metadata_raw));

    tracing::info!("running ffmpeg");
    let ffmpeg = std::process::Command::new("ffmpeg")
        .args(pre_args)
        .arg("-i")
        .arg("-")
        .args(&ffmpeg_args)
        .stdin(deemix.stdout.take().ok_or(SongbirdError::Stdout)?)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start child process");
    
    tracing::info!("deezer metadata {:?}", metadata);
    let ffmpeg_ptr = ffmpeg.stdout.as_ref().ok_or(SongbirdError::Stdout)?.as_raw_fd();
    unsafe { bigpipe(ffmpeg_ptr, pipesize); }
    
    let now = std::time::Instant::now();
    let pipesize = max_pipe_size().await.unwrap();
    let pipe_threshold = std::env::var("MKBIRD_PIPE_THRESHOLD")
        .unwrap_or_else(|_| "0.8".to_string())
        .parse::<f32>()
        .unwrap_or(0.8);

    loop {
        let avail = unsafe { availbytes(ffmpeg_ptr) };            
        let mut percentage = 0.0;
        if 0 > avail {
            break
        }
        if avail > 0 {
            percentage = pipesize as f32 / avail as f32;
        }

        if pipe_threshold > percentage {
            tokio::time::sleep(std::time::Duration::from_micros(200)).await;
            tracing::debug!("availbytes: {}", avail);
            tracing::debug!("pipesize: {}", pipesize);
        }
        else {
            tracing::info!("load time: {}", now.elapsed().as_secs_f64());
            tracing::debug!("availbytes: {}", avail);
            tracing::debug!("pipesize: {}", pipesize);
            break
        }
    }  
 
    Ok(Input::new(
        true,
        children_to_reader::<f32>(vec![deemix, ffmpeg]),
        Codec::FloatPcm,
        Container::Raw,
        metadata,
    ))
}

fn metadata_from_deemix_output(val: &serde_json::Value) -> Metadata
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

    let source_url = obj
        .and_then(|m| m.get("link"))
        .and_then(Value::as_str)
        .map(str::to_string);

    Metadata {
        track,
        artist,
        channels: Some(2),
        duration,
        source_url,
        sample_rate: Some(SAMPLE_RATE_RAW as u32),
        ..Default::default()
    }
}
