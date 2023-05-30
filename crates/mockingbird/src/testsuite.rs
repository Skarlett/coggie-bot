use std::env::var;
use std::path::PathBuf;

fn binexists(file: &str) {
    let paths = var("PATH").unwrap();
    assert!(paths.split(':').filter(|p| PathBuf::from(p).join(file).exists()).count() == 1);
}

#[test]
#[cfg(feature="deemix")]
fn path_deemix() {
    binexists("deemix")
}

#[test]
#[cfg(feature="deemix")]
fn path_stream_deemix() {
    binexists("stream-deemix")
}

#[test]
fn path_ffmpeg() {
    binexists("ffmpeg")
}

#[test]
#[cfg(feature="ytdl")]
fn path_ytdl() {
   binexists("yt-dlp")
}
