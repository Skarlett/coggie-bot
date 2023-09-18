#[link(name = "fion")]
extern {
    pub fn availbytes(fd: std::ffi::c_int) -> std::ffi::c_int;
    pub fn bigpipe(fd: std::ffi::c_int, size: std::ffi::c_int) -> std::ffi::c_int;
}
#[derive(Debug)]
pub enum PipeError {
    ImmutableSize,
    InvalidSize,
    IOError(std::io::Error),
    ParseIntError(std::num::ParseIntError),
    NoMaxPipeSize,
}

impl std::fmt::Display for PipeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PipeError::ImmutableSize => write!(f, "Pipe size is immutable"),
            PipeError::InvalidSize => write!(f, "Invalid pipe size"),
            PipeError::IOError(e) => write!(f, "IO error: {}", e),
            PipeError::ParseIntError(e) => write!(f, "Parse int error: {}", e),
            PipeError::NoMaxPipeSize => write!(f, "No max pipe size"),
        }
    }
}


impl From<std::io::Error> for PipeError {
    fn from(err: std::io::Error) -> Self {
        PipeError::IOError(err)
    }
}

impl From<std::num::ParseIntError> for PipeError {
    fn from(err: std::num::ParseIntError) -> Self {
        PipeError::ParseIntError(err)
    }
}

#[cfg(feature="tokio")]
pub async fn max_pipe_size() -> Result<i32, PipeError> {
    use tokio::io::AsyncReadExt;
    
    let mut file = tokio::fs::OpenOptions::new()
        .read(true)
        .open("/proc/sys/fs/pipe-max-size")
        .await?;
    
    let mut buf = String::new();
    file.read_to_string(&mut buf).await?; 
    
    let data = buf.trim();
    Ok(data.parse::<i32>()?)
}

#[cfg(feature="stdio")]
pub fn std_max_pipe_size() -> Result<i32, PipeError> {
    use std::io::Read;

    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .open("/proc/sys/fs/pipe-max-size")?;
    
    let mut buf = String::new();
    file.read_to_string(&mut buf)?; 
    
    let data = buf.trim();
    Ok(data.parse::<i32>()?)
}