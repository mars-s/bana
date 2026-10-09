//! `tanacraft-mcp [--file FILE | --connect HOST:PORT]`: MCP server on stdio.
//! Default file: `~/TanaCraft/graph.json`. With `--connect`, drives a running app.

use tanacraft_commands::Session;
use tanacraft_core::Graph;
use tanacraft_mcp::{Backend, Server};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let backend = match opt("--connect") {
        Some(addr) => Backend::connect(&addr)?,
        None => {
            let path = opt("--file").map(std::path::PathBuf::from).unwrap_or_else(tanacraft_core::default_graph_path);
            let graph = if path.exists() { Graph::load(&path).map_err(|e| e.to_string())? } else { Graph::new() };
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let mut s = Session::new(graph);
            s.path = Some(path);
            Backend::Local(Box::new(s))
        }
    };
    let stdin = std::io::stdin();
    Server::new(backend).serve(stdin.lock(), std::io::stdout().lock()).map_err(|e| e.to_string())
}
