//! `tanacraft-mcp [--file FILE] [--connect HOST:PORT --token TOKEN]`: MCP server on stdio.
//! Works on the running app that has FILE open (default `~/TanaCraft/graph.tanacraft`), found
//! through the live file beside it; on the file itself only when no app has it open.

use tanacraft_mcp::{Backend, Server};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let backend = match opt("--connect") {
        Some(addr) => Backend::connect(&addr, opt("--token"))?,
        None => Backend::open(&opt("--file").map(std::path::PathBuf::from).unwrap_or_else(tanacraft_core::default_graph_path))?,
    };
    let stdin = std::io::stdin();
    Server::new(backend).serve(stdin.lock(), std::io::stdout().lock()).map_err(|e| e.to_string())
}
