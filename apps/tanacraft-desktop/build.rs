fn main() {
    // The web UI is embedded from web/dist. A fresh checkout has no build yet: put a page there
    // that says how to make one, so `cargo build` and `cargo clippy` work without node.
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    if !dist.join("index.html").exists() {
        let _ = std::fs::create_dir_all(&dist);
        let _ = std::fs::write(
            dist.join("index.html"),
            "<!doctype html><p>The web UI is not built: run <code>pnpm --dir web install && pnpm --dir web build</code>, then build the app again.</p>",
        );
    }
    tauri_build::build()
}
