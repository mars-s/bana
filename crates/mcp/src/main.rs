//! `bana-mcp [--file FILE] [--connect HOST:PORT --token TOKEN]`: MCP server on stdio.
//! Works on the running app that has FILE open (default `~/Bana/graph.bana`), found
//! through the live file beside it; on the file itself only when no app has it open.

use bana_mcp::{Backend, Server};

const USAGE: &str = "usage: bana-mcp [--file FILE] [--connect 127.0.0.1:PORT [--token TOKEN]]

An MCP server (JSON-RPC on stdin and stdout) for Bana notes.
  --file FILE        the graph (default ~/Bana/graph.bana); the running app that has it
                     open is used when there is one, else the file itself
  --connect ADDR     a running app at 127.0.0.1:PORT
  --token TOKEN      its token (default: this install's, ~/Bana/token)
  -h, --help         this text
Which graph or app it works on is written to stderr when it starts.";

fn main() {
    if let Err(e) = run() {
        eprintln!("bana-mcp: {e}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    // Every flag takes a value; anything else is a mistake worth saying (DIFFS N89).
    let mut opts = std::collections::HashMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--file" | "--connect" | "--token" => {
                let v = it.next().ok_or(format!("{a} needs a value\n{USAGE}"))?;
                opts.insert(a.as_str(), v.clone());
            }
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    bana_core::rename::migrate_home().map_err(|e| e.to_string())?;
    let backend = match opts.get("--connect") {
        Some(addr) => Backend::connect(addr, opts.get("--token").cloned())?,
        None => Backend::open(&opts.get("--file").map(std::path::PathBuf::from).unwrap_or_else(bana_core::default_graph_path))?,
    };
    eprintln!("bana-mcp: working on {}", backend.describe());
    let stdin = std::io::stdin();
    Server::new(backend).serve(stdin.lock(), std::io::stdout().lock()).map_err(|e| e.to_string())
}
