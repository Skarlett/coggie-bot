pub mod controller;
mod extractor;
use serenity::client::ClientBuilder;

pub async fn init(cfg: ClientBuilder) -> ClientBuilder {
    tracing::info!("Mockingbird initialized");
    return extractor::init(cfg).await;
}

#[cfg(test)]
mod tests {

    use std::env::var;
    use std::path::PathBuf;

    #[test]
    fn path_ffmpeg() {
        let paths = var("PATH").unwrap();
        assert!(paths.split(':').filter(|p| PathBuf::from(p).join("ffmpeg").exists()).count() >= 1);
    }
}
