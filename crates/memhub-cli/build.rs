// rust-embed needs `ui/dist` to exist at compile time. Create a placeholder so
// `cargo build` works before the web UI has been built (run `npm run build` in ui/).
fn main() {
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist");
    if !dist.join("index.html").exists() {
        let _ = std::fs::create_dir_all(&dist);
        let _ = std::fs::write(
            dist.join("index.html"),
            "<!doctype html><title>MemHub</title><p>Web UI not built. Run <code>npm install && npm run build</code> in <code>ui/</code>, then rebuild.</p>",
        );
    }
    println!("cargo:rerun-if-changed=build.rs");
}
