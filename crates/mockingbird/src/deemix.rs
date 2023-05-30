use songbird::{
    constants::SAMPLE_RATE_RAW,
    input::{
        children_to_reader,
        error::{Error as SongbirdError, Result as SongbirdResult},
        Codec,
        Container,
        Metadata,
        Input,
        restartable::Restart
    },
};
use std::{
    io::{BufReader, BufRead, Read},
    process::Stdio,
    time::Duration
};
use serde_json::Value;
use std::os::fd::AsRawFd;
use tokio::io::AsyncReadExt;

#[tracing::instrument]
async fn max_pipe_size() -> Result<i32, Box<dyn std::error::Error>> {
    let mut file = tokio::fs::OpenOptions::new()
        .read(true)
        .open("/proc/sys/fs/pipe-max-size")
        .await?;
    
    let mut buf = String::new();
    file.read_to_string(&mut buf).await?; 
    
    tracing::info!("max pipe size: {}", buf);
    let data = buf.trim();
    Ok(data.parse::<i32>()?)
}


#[link(name = "fion")]
extern {
    fn availbytes(fd: std::ffi::c_int) -> std::ffi::c_int;
    fn bigpipe(fd: std::ffi::c_int, size: std::ffi::c_int) -> std::ffi::c_int;
}

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
            _deemix(self.uri.as_ref(), &["-ss", &ts]).await
        } else {
            deemix(self.uri.as_ref()).await
        }
    }

    async fn lazy_init(&mut self) -> Result<(Option<Metadata>, Codec, Container), SongbirdError> {
        Ok(( Some(deemix_metadata(self.uri.as_ref()).await.unwrap()), Codec::FloatPcm, Container::Raw))
    }
}

pub async fn deemix_metadata(uri: &str) -> Result<Metadata, Box<dyn std::error::Error>> {
    let deemix = tokio::process::Command::new("deemix-metadata")
        .arg(uri.trim())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let output = deemix.wait_with_output().await?;
    
    Ok(metadata_from_deemix_output(&serde_json::from_slice(&output.stdout[..])?))
}


pub async fn deemix(
    uri: &str,
) -> SongbirdResult<Input>{ 
    _deemix(uri, &[]).await
}

// #[tracing::instrument]
pub async fn _deemix(
    uri: &str,
    pre_args: &[&str],
) -> SongbirdResult<Input>
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
        .arg(uri.trim())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    unsafe { 
        let raw_stdout = deemix.stdout.as_ref().unwrap().as_raw_fd();
        bigpipe(raw_stdout, pipesize);
    }

    let stderr = deemix.stderr.take();
    let (returned_stderr, value) = tokio::task::spawn_blocking(move || {
        let mut s = stderr.unwrap();
        let out: SongbirdResult<Value> = {
            let mut o_vec = vec![];
            let mut serde_read = BufReader::new(s.by_ref());
            // Newline...
            if let Ok(len) = serde_read.read_until(0xA, &mut o_vec) {
                serde_json::from_slice(&o_vec[..len]).map_err(|err| SongbirdError::Json {
                    error: { 
                        tracing::error!("TRIED PARSING \n {}", String::from_utf8_lossy(&o_vec));
                        err
                    },
                    parsed_text: std::str::from_utf8(&o_vec).unwrap_or_default().to_string(),
                })
            } else {
                tracing::error!("TRIED PARSING \n {}", String::from_utf8_lossy(&o_vec));
                SongbirdResult::Err(SongbirdError::Metadata)
            }
        };

        (s, out)
    })
    .await
    .map_err(|_| SongbirdError::Metadata)?;

    deemix.stderr = Some(returned_stderr);
    
    let taken_stdout = deemix.stdout.take().ok_or(SongbirdError::Stdout)?;
    tracing::info!("running ffmpeg");
    let ffmpeg = std::process::Command::new("ffmpeg")
        .args(pre_args)
        .arg("-i")
        .arg("-")
        .args(&ffmpeg_args)
        .stdin(taken_stdout)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start child process");
    
    let metadata_raw = value?;
    
    let metadata = Some(metadata_from_deemix_output(&metadata_raw));

    let totalbytes = metadata_raw["filesize"].as_i64().unwrap();

    tracing::info!("deezer metadata {:?}", metadata);
    let ptr = ffmpeg.stdout.as_ref().unwrap().as_raw_fd();
    unsafe {
        bigpipe(ptr, pipesize);
    }
    let now = std::time::Instant::now();
    loop {
        let avail = unsafe { availbytes(ptr) };            
        let mut percentage = 0.0;
        // collect atleast 25% of the data before starting
        if 0 > avail {
            break
        }
        
        if avail > 0 {
            percentage = pipesize as f32 / avail as f32;
        }

        if 0.8 > percentage {
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
    
    Metadata {
        track,
        artist,
        channels: Some(2),
        duration,
        sample_rate: Some(SAMPLE_RATE_RAW as u32),
        ..Default::default()
    }
}