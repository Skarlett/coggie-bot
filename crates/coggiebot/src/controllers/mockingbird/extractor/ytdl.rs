pub fn is_ytdl(uri: &str) -> bool {
    const YTDL_HANDLES: [&'static str; 3] = [
        "youtube.com",
        "youtu.be",
        "soundcloud.com",
    ];
    YTDL_HANDLES.iter().any(|x| uri.contains(x))
}

#[cfg(tests)]
mod tests {
    use std::env::var;
    use std::path::PathBuf;

    #[test]
    #[cfg(feature="mockingbird-ytdl")]
    fn path_ytdl() {
        let paths = var("PATH").unwrap();
        assert!(paths.split(':').filter(|p| PathBuf::from(p).join("yt-dlp").exists()).count() == 1);
    }
}
