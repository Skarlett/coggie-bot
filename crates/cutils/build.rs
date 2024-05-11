// use std::path::Path;

fn main() {
    cc::Build::new()
        .file("src/fion.c")
        .compile("fion");
    
    println!("cargo:rustc-link-lib=static=fion");
    println!("cargo:rerun-if-changed=src/fion.c");
}