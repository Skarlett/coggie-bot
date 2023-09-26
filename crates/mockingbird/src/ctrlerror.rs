use songbird::input::error::Error as SongbirdError;

#[allow(unused_variables)]
#[derive(Debug)]
pub enum HandlerError {
    Songbird(SongbirdError),
    IOError(std::io::Error),
    Serenity(serenity::Error),
    
    #[cfg(feature = "deemix")]
    DeemixError(crate::deemix::DeemixError),
    NotImplemented,
    NoCall
}


impl From<serenity::Error> for HandlerError {
    fn from(err: serenity::Error) -> Self {
        HandlerError::Serenity(err)
    }
}

impl From<SongbirdError> for HandlerError {
    fn from(err: SongbirdError) -> Self {
        HandlerError::Songbird(err)
    }
}

impl From<std::io::Error> for HandlerError {
    fn from(err: std::io::Error) -> Self {
        HandlerError::IOError(err)
    }
}

#[cfg(feature = "deemix")]
impl From<crate::deemix::DeemixError> for HandlerError {
    fn from(err: crate::deemix::DeemixError) -> Self {
        HandlerError::DeemixError(err)
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Songbird(err) => write!(f, "Songbird error: {}", err),
            Self::NotImplemented => write!(f, "This feature is not implemented."),
            
            Self::IOError(err)
                => write!(f, "IO error: (most likely deemix-metadata failed) {}", err),
            
            Self::Serenity(err)
                => write!(f, "Serenity error: {}", err),
            
            Self::NoCall
                => write!(f, "Not in a voice channel to play in"),
            
            #[cfg(feature = "deemix")]
            Self::DeemixError(crate::deemix::DeemixError::BadJson(err))
                => write!(f, "Deemix error: {}", err),

            _ => write!(f, "Unknown error")
        }
    }
}
impl std::error::Error for HandlerError {}

