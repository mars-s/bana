//! Runs requests on the sample graph (in a temp file) and prints the answers, to see the
//! snapshot shapes the phone draws: `cargo run -p bana-ffi --example rpc -- page '{}' run '{"command":"nav.supertags"}' page '{}'`
fn main() {
    let path = std::env::temp_dir().join("bana-ffi-example.bana");
    let _ = std::fs::remove_file(&path);
    let Ok(e) = bana_ffi::Engine::open(path.display().to_string(), true, Some("2026-10-10".into())) else { return };
    let args: Vec<String> = std::env::args().skip(1).collect();
    for pair in args.chunks(2) {
        let (m, p) = (pair[0].clone(), pair.get(1).cloned().unwrap_or_default());
        println!("{}", e.rpc(m, p));
    }
}
