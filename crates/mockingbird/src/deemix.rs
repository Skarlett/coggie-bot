use std::{
    io::{BufReader, BufRead, Read},
    os::fd::RawFd,
    mem,
    process::{Child, Stdio},
    time::Duration
};
use songbird::{
    constants::SAMPLE_RATE_RAW,
    input::{
        children_to_reader,
        error::Error as SongbirdError,
        Codec,
        Container,
        Metadata,
        Input,
        restartable::Restart,
        Reader,
    },
};
use serde_json::Value;
use std::os::fd::AsRawFd;
use tokio::io::AsyncReadExt;
use cutils::{availbytes, bigpipe, max_pipe_size, PipeError};
use tokio::runtime::Handle;
use tracing::debug;

#[derive(Debug)]
pub struct PreloadChildContainer(pub Vec<Child>);

impl PreloadChildContainer {
    /// Create a new [`ChildContainer`] from a child process
    pub fn new(children: Vec<Child>) -> Self {
        Self(children)
    }

    pub fn inner(mut self) -> Vec<Child> {
        self.0.drain(..).collect()
    }
}

impl Drop for PreloadChildContainer {
    fn drop(&mut self) {
        let children: Vec<_> = self.0.drain(..).collect();

        if let Ok(handle) = Handle::try_current() {
            handle.spawn_blocking(move || {
                cleanup_child_processes(children);
            });
        } else {
            cleanup_child_processes(children);
        }
    }
}

impl From<PreloadChildContainer> for Reader {
    fn from(value: PreloadChildContainer) -> Self {
        children_to_reader::<f32>(value.inner())
    }
}

fn cleanup_child_processes(mut children: Vec<Child>) {
    let attempt = if let Some(child) = children.pop() {
        child.wait_with_output()
    } else {
        return;
    };

    let attempt = attempt.and_then(|_| {
        children
            .iter_mut()
            .rev()
            .try_for_each(|child| child.wait().map(|_| ()))
    });

    if let Err(e) = attempt {
        debug!("Error awaiting child process: {:?}", e);
    }
}


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
            _deemix(self.uri.as_ref(), &["-ss", &ts], true)
                .await
                .map_err(DeemixError::into)
                .map(|(i, _)| i)
        } else {
            deemix(self.uri.as_ref(), true)
                .await
                .map_err(DeemixError::into)
                .map(|(i, _)| i)
        }
    }

    async fn lazy_init(&mut self) -> Result<(Option<Metadata>, Codec, Container), SongbirdError> {
        Ok(
        (
            Some(deemix_metadata(self.uri.as_ref())
                    .await
                    .map(DeemixMetadata::into)
                    .map_err(SongbirdError::from)?
            ),
            Codec::FloatPcm, Container::Raw)
        )
    }
}


pub async fn deemix_metadata(uri: &str) -> std::io::Result<DeemixMetadata> {
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
    balloon: bool,
) -> Result<(Input, Option<DeemixMetadata>), DeemixError> {
    _deemix(uri, &[], balloon)
        .await
}

async fn _deemix_stream(uri: &str, pipesize: i32) -> Result<(std::process::Child, DeemixMetadata), DeemixError> 
{  
    let mut deemix = std::process::Command::new("deemix-stream")
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

    let (returned_stderr, metadata_raw) = threadout;

    deemix.stderr = Some(returned_stderr);
    
    let metadata_raw = metadata_raw?;
    if let Some(_) = metadata_raw.get("error") {
        return Err(DeemixError::Metadata);
    }

    let _filesize = metadata_raw["filesize"].as_u64();

    Ok((deemix, metadata_from_deemix_output(&metadata_raw)))
}

fn _balloon_loader(proc: &mut std::process::Child, pipesize: i32) -> Result<std::process::Child, DeemixError> {
    let balloon = std::process::Command::new("balloon")
        .stdin(
            proc.stdout.take()
                .ok_or(SongbirdError::Stdout)?       
        )
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start child process");

    let balloon_ptr = balloon.stdout.as_ref()
        .ok_or(SongbirdError::Stdout)?
        .as_raw_fd();
    
    unsafe { bigpipe(balloon_ptr, pipesize); }
    
    Ok(balloon)
}

fn _ffmpeg(proc: &mut std::process::Child, pre_args: &[&str], pipesize: i32) -> Result<std::process::Child, DeemixError> { 
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
 
    let ffmpeg = std::process::Command::new("ffmpeg")
        .args(pre_args)
        .arg("-i")
        .arg("-")
        .args(&ffmpeg_args)
        .stdin(
            proc.stdout
                .take()
                .ok_or(SongbirdError::Stdout)?       
        )
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start child process");
    
    let ffmpeg_ptr = ffmpeg.stdout.as_ref()
        .ok_or(SongbirdError::Stdout)?
        .as_raw_fd();
    
    unsafe { bigpipe(ffmpeg_ptr, pipesize); }
    
    Ok(ffmpeg)
}

pub struct PreloadInput {
    ffmpeg_stdout: RawFd,
    pub children: PreloadChildContainer,
    pub metadata: Option<DeemixMetadata>,
    balloon: bool,
}

pub async fn _deemix_preload(
    uri: &str,
    pre_args: &[&str],
    balloon: bool,
    pipesize: i32
) -> Result<PreloadInput, DeemixError>
{
    let mut children = Vec::with_capacity(3);

    tracing::info!("Running: deemix-stream {} {}", pre_args.join(" "), uri);
    let (mut deemix, metadata) =  _deemix_stream(uri, pipesize).await?;

    let mut balloon_proc = if balloon {
        tracing::info!("running balloon");
        Some(_balloon_loader(&mut deemix, pipesize)?)
    } else { None };
    
    let output = balloon_proc.as_mut()
        .unwrap_or(&mut deemix);
    
    let ffmpeg = _ffmpeg(output, pre_args, pipesize)?;
 
    children.push(deemix);

    if let Some(balloon) = balloon_proc {
        children.push(balloon);
    }

    children.push(ffmpeg);

    let stdout_fd = children.last().unwrap().stdout.as_ref()
        .ok_or(SongbirdError::Stdout)?
        .as_raw_fd();

    return Ok(PreloadInput {
        balloon,
        ffmpeg_stdout: stdout_fd,
        children: PreloadChildContainer::new(children),
        metadata: Some(metadata),
    })
}


pub async fn _deemix(
    uri: &str,
    pre_args: &[&str],
    balloon: bool,
) -> Result<(Input, Option<DeemixMetadata>), DeemixError>
{
    let pipesize = max_pipe_size().await.unwrap();
    
    let mut preload_input = _deemix_preload(uri, pre_args, balloon, pipesize).await?;
    let now = std::time::Instant::now();

    let pipe_threshold = std::env::var("MKBIRD_PIPE_THRESHOLD")
        .unwrap_or_else(|_| "0.8".to_string())
        .parse::<f32>()
        .unwrap_or(0.8);

    loop {
        let avail = unsafe { availbytes(preload_input.ffmpeg_stdout) };
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

    Ok((
        Input::new(
        true,
        children_to_reader::<f32>(preload_input.children.inner()),
        Codec::FloatPcm,
        Container::Raw,
        preload_input.metadata.clone().map(|x| x.into()),
    ), 
    preload_input.metadata.clone()))
}

#[derive(Debug, Clone)]
pub struct DeemixMetadata {
    pub isrc: Option<String>,
    pub metadata: Metadata,
}

impl Into<Metadata> for DeemixMetadata {
    fn into(self) -> Metadata {
        self.metadata
    }
}

fn metadata_from_deemix_output(val: &serde_json::Value) -> DeemixMetadata
{
    let obj = val.as_object();

    let track = obj
        .and_then(|m| m.get("title"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .clone();

    let artist = obj
        .and_then(|m| m.get("artist"))
        .and_then(|x| x.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .clone();
 
   let duration = obj
        .and_then(|m| m.get("duration"))
        .and_then(Value::as_f64)
        .map(Duration::from_secs_f64)
        .clone();

   let source_url = obj
        .and_then(|m| m.get("link"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .clone();

    let isrc = obj
        .and_then(|m| m.get("isrc"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .clone();

    DeemixMetadata {
        isrc,
        metadata: Metadata {
            track,
            artist,
            channels: Some(2),
            duration,
            source_url,
            sample_rate: Some(SAMPLE_RATE_RAW as u32),
            ..Default::default()
        }
    }
}
