use std::{
    io::{BufReader, BufRead, Read},
    process::{Child, Stdio},
    time::Duration
};
use serenity::futures::io::BufWriter;
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
use cutils::{availbytes, bigpipe, max_pipe_size, PipeError};
use tokio::runtime::Handle;
use tracing::debug;


#[derive(Debug)]
pub enum DeemixError {
    BadJson(String),
    Metadata,
    IO(std::io::Error),
    ParseInt(core::num::ParseIntError),
    Songbird(SongbirdError),
    Tokio(tokio::task::JoinError),
}

#[derive(Debug)]
pub enum DeemixLoadMethod {
    Mem,
    Child,
}

#[derive(Debug)]
pub enum FFmpegInput {
    Child(std::process::Child),
    Mem(Option<Vec<u8>>),

    //Disk(std::path::PathBuf),
}

impl FFmpegInput {
    pub fn into_inner(self) -> Result<std::process::Child, DeemixError> {
        match self {
            FFmpegInput::Child(child) => Ok(child),
            FFmpegInput::Mem(_) => panic!("cannot call into_inner on FFmpegInput::Mem"),
        }
    }
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
            _deemix(self.uri.as_ref(), &["-ss", &ts], true, DeemixLoadMethod::Mem)
                .await
                .map_err(DeemixError::into)
                .map(|(i, _)| i)
        } else {
            deemix(self.uri.as_ref())
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
) -> Result<(Input, Option<DeemixMetadata>), DeemixError> {
    _deemix(uri, &[], true, DeemixLoadMethod::Mem).await
}

async fn _deemix_stream(uri: &str, pipesize: i32, method: DeemixLoadMethod) -> Result<(FFmpegInput, DeemixMetadata), DeemixError> 
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

    let stdout = deemix.stdout.take();
    let _filesize = metadata_raw["filesize"].as_u64();
    let (deemix_stdout, output) = match method {
        DeemixLoadMethod::Mem => {
            let (stdout, buf) = tokio::task::spawn_blocking(|| {
                let mut buf = Vec::new();
                let mut stdout = stdout.unwrap();
                let mut reader = BufReader::new(&mut stdout);
                reader.read_to_end(&mut buf).unwrap();
                (stdout, buf) 
            }).await?;
            
            (stdout, FFmpegInput::Mem(Some(buf)))
        },
        DeemixLoadMethod::Child => return Ok((FFmpegInput::Child(deemix), metadata_from_deemix_output(&metadata_raw))),
    };

    deemix.stdout = Some(deemix_stdout);

    Ok((output, metadata_from_deemix_output(&metadata_raw)))
}

fn _ffmpeg(proc: &mut FFmpegInput, pre_args: &[&str], pipesize: i32) -> Result<std::process::Child, DeemixError> {
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

    let mut output: Stdio = Stdio::piped();

    if let FFmpegInput::Child(child) = proc {
        output = child.stdin
            .take()
            .ok_or(SongbirdError::Stdout)
            .map(|x| x.into())?
    }

    let mut ffmpeg = std::process::Command::new("ffmpeg")
        .args(pre_args)
        .arg("-i")
        .arg("-")
        .args(&ffmpeg_args)
        .stdin(output)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start child process");
    
    let ffmpeg_ptr = ffmpeg.stdout.as_ref()
        .ok_or(SongbirdError::Stdout)?
        .as_raw_fd();
    
    unsafe { bigpipe(ffmpeg_ptr, pipesize); }

    let stdin = ffmpeg.stdin.take();

    if let FFmpegInput::Mem(opt_buf) = proc {
        let buf = Option::take(opt_buf);
        tokio::task::spawn_blocking(move || {
            use std::io::Write;
            std::io::BufWriter::new(stdin.unwrap())
            .write_all(&buf.unwrap()[..])
            .unwrap();
        });
    }

    Ok(ffmpeg)
}

pub async fn _deemix(
    uri: &str,
    pre_args: &[&str],
    wait: bool,
    load_method: DeemixLoadMethod,
) -> Result<(Input, Option<DeemixMetadata>), DeemixError>
{
    let pipe_threshold = std::env::var("MKBIRD_PIPE_THRESHOLD")
        .unwrap_or_else(|_| "0.8".to_string())
        .parse::<f32>()
        .unwrap_or(0.8);

    let pipesize = max_pipe_size().await.unwrap();

    tracing::info!("Running: deemix-stream {} {}", pre_args.join(" "), uri);
    let (mut deemix, metadata) =  _deemix_stream(uri, pipesize, load_method).await?;

    let ffmpeg = _ffmpeg(&mut deemix, pre_args, pipesize)?;
    let stdout_fd = ffmpeg.stdout.as_ref()
        .ok_or(SongbirdError::Stdout)?
        .as_raw_fd();

    if wait {
        let now = std::time::Instant::now();
        loop {
            let avail = unsafe { availbytes(stdout_fd) };
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
    }


    let children = match deemix {
        FFmpegInput::Child(child) =>
            children_to_reader::<f32>(vec![child, ffmpeg]),
        FFmpegInput::Mem(_) =>
            children_to_reader::<f32>(vec![ffmpeg])
    };
    
    Ok((
        Input::new(
            true,
            children,
            Codec::FloatPcm,
            Container::Raw,
            Some(metadata.clone().into()),
        ),
        Some(metadata.clone())
    ))
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
