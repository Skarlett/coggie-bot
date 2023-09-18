use std::os::fd::AsRawFd;
use std::io::{Read, Write};
use cutils::{availbytes, bigpipe, std_max_pipe_size};

const CHUNK_SIZE: usize = 4096;
const BUFFER_SIZE: usize = 1024^3 * 5;


fn main() {
    let pipesize = std_max_pipe_size()
        .unwrap();
    
    let mut chunk = [0u8; CHUNK_SIZE];
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);
    let mut reader = std::io::stdin();
    let mut writer = std::io::stdout();

    unsafe {
        bigpipe(writer.as_raw_fd(), pipesize);
        bigpipe(reader.as_raw_fd(), pipesize);
    }

    loop {        
        let n = reader.read(&mut chunk).unwrap();
        
        let request_write = unsafe { 
             availbytes(writer.as_raw_fd())           
        };

        if buffer.is_empty() && request_write as usize > n {
            writer.write(&chunk[..n]).unwrap();
            continue;
        }

        if request_write > 0 {
            let slice = &buffer[..request_write as usize];
            writer.write(slice).unwrap(); 
        }

        if n > 0 {
            buffer.extend_from_slice(&chunk[..n]);
        }
        
        if n == 0 && request_write == pipesize && buffer.is_empty() {
            break;
        }
    }
}