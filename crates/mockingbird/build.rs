fn main() {
    if std::env::var("CARGO_FEATURE_DEEMIX").is_ok() {
        cc::Build::new()
            .file("src/fion.c")
            .compile("fion");
        println!("cargo:rustc-link-lib=static=fion");
    }    
}