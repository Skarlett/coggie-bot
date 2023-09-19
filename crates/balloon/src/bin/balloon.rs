use std::io::{Read, Write};

const CHUNK_SIZE: usize = 4096 * 4;
const BUFFER_SIZE: usize = 1024^2 * 5;

fn main() -> Result<(), std::io::Error> {
    let mut chunk = [0u8; CHUNK_SIZE];
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);
    let mut reader = std::io::stdin();
    let mut writer = std::io::stdout();

    while let Ok(n) = reader.read(&mut chunk) {
        if n == 0 {
            break;
        }  
        buffer.extend(&chunk[..n]);
    }

    for it in buffer.chunks(CHUNK_SIZE) {
        // TODO: I have no idea if this works, 
        // but it's a start. (I think it does work.)
        chunk.copy_from_slice(it);
        writer.write_all(&chunk)?;
    }

    return Ok(())
}