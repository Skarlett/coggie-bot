use std::io::Read;

/// Just used in testing to simulate a slow reader.
fn main() {
    let mut stdin = std::io::stdin();    
    let mut chunk = [0u8; 1024];

    loop {
        let n = stdin.read(&mut chunk).unwrap();
        println!("[SLOWREAD] read: {}", n);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}