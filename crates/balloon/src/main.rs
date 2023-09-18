use std::os::fd::AsRawFd;
use std::io::{Read, Write};
use cutils::{availbytes, bigpipe, std_max_pipe_size, PipeError};

const CHUNK_SIZE: usize = 4096;
const BUFFER_SIZE: usize = 1024^3 * 5;


fn main() -> Result<(), PipeError> {
    let pipesize = std_max_pipe_size()
        .unwrap();
    
    let mut chunk = [0u8; CHUNK_SIZE];
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);
    let mut reader = std::io::stdin();
    let mut writer = std::io::stdout();

    unsafe {
        #[cfg(feature="debug")]
        eprintln!("pipesize: {}", pipesize);

        bigpipe(writer.as_raw_fd(), pipesize);
        bigpipe(reader.as_raw_fd(), pipesize);
    }

    loop {        
        let n = reader.read(&mut chunk).unwrap();
        let request_write = unsafe { 
            availbytes(writer.as_raw_fd())           
        };

        #[cfg(feature="debug")]
        eprintln!("requesting: {}", request_write);

        let request_write = match request_write {
            0 => 0u64,
            -1 => return Err(PipeError::NoMaxPipeSize),
            -2 => return Err(PipeError::ImmutableSize),
            x => 0u64.saturating_add(x as u64)
        };

        if buffer.is_empty() && request_write as usize > n {           
            #[cfg(feature="debug")]
            eprintln!("direct write: {}", n);
            
            writer.write(&chunk[..n])?;
            continue;
        }

        if request_write > 0 {
            let drain = request_write.min(buffer.len() as u64) as usize;
            let slice: Vec<_> = buffer.drain(..drain).collect();

            #[cfg(feature="debug")]
            eprintln!("write from balloon: {}", slice.len());
            writer.write(&slice[..])?; 
        }

        if n > 0 {
            #[cfg(feature="debug")]
            eprintln!("bigger balloon: {}", n);
            buffer.extend_from_slice(&chunk[..n]);
        }
        
        if n == 0 && request_write == pipesize as u64 && buffer.is_empty() {
            #[cfg(feature="debug")]
            eprintln!("deflated balloon");
            return Ok(());
        }
    }
}