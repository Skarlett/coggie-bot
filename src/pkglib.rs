use derive_builder::Builder;
use std::path::PathBuf;
use thiserror::Error;
use std::io::{BufReader, BufRead};

#[derive(Debug, Error)]
pub enum CoggiebotError {

    #[error("Error while sending request to Discord: {0}")]
    UserMessage(String),

    #[error("Serenity error: {0}")]
    SerenityError(#[from] serenity::Error),

    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
}
