use std::io::{Read, Write};

const CHUNK_SIZE: usize = 4096 * 4;
const BUFFER_SIZE: usize = 1024^2 * 5;

fn main() -> Result<(), std::io::Error> {
    let mut chunk = [0u8; CHUNK_SIZE];
    let mut buffer = dbg!(Vec::with_capacity(BUFFER_SIZE));
    let mut reader = std::io::stdin();
    let mut writer = std::io::stdout();

    while let Ok(n) = reader.read(&mut chunk) {
        if n == 0 {
            break;
        }  
        buffer.extend(&chunk[..n]);
    }

    for it in buffer.chunks(CHUNK_SIZE) {
        chunk.copy_from_slice(it);
        writer.write_all(&chunk)?;
    }

    return Ok(())
}