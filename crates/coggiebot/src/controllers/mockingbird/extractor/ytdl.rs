use songbird::input::Restartable;

pub fn is_ytdl(uri: &str) -> bool {
    const YTDL_HANDLES: [&'static str; 3] = [
        "youtube.com",
        "youtu.be",
        "soundcloud.com",
    ];
    YTDL_HANDLES.iter().any(|x| uri.contains(x))
}

pub async fn ytdl(uri: String) -> Result<Restartable, songbird::input::error::Error> {
    Restartable::ytdl(uri, true).await
}