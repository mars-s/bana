//! TanaCraft command line. Every subcommand works on a graph file (default `~/TanaCraft/graph.json`,
//! or `--file FILE`) and runs the same commands as the app.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::{Value, json};
use tanacraft_commands::Session;
use tanacraft_core::{Date, Graph, NodeId, PRODUCT_NAME, sample};

const USAGE: &str = "usage: tanacraft-cli [--file FILE] <command> [args]

commands:
  inspect [NODE]                    a node and its subtree as JSON (default: Home)
  view [NODE]                       the outline as text
  add NAME [--parent NODE | --today] [--tag TAG]...
                                    add a node; prints its id
  capture TEXT [--tag TAG]          add a node to the capture inbox; prints its id
  import FILE [--parent NODE]       import a Markdown (.md) or OPML (.opml) outline (default: into the Library)
  export [NODE] [--format md|json|opml]  export a subtree (default: Markdown of Home)
  search QUERY                      find nodes
  tags                              list supertags with their fields
  run --cmd 'ID={json}' ...         run commands in order, then save
  commands                          list every command (id, shortcut, params)
  sample [--out FILE]               write the sample graph
  mcp [--connect HOST:PORT]         MCP server on stdio
  serve [--port PORT] [--sample] [--web DIR] [--today YYYY-MM-DD]
                                    serve the web UI and its JSON API on 127.0.0.1 (default port 8787);
                                    --sample without --file keeps the sample in memory only

NODE is a node id; `today` means today's day node.";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(out) => {
            if !out.is_empty() {
                println!("{out}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Removes `--name VALUE` from `args` and returns VALUE.
fn take_opt(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.remove(i);
    (i < args.len()).then(|| args.remove(i))
}

fn take_flag(args: &mut Vec<String>, name: &str) -> bool {
    let found = args.iter().position(|a| a == name);
    if let Some(i) = found {
        args.remove(i);
    }
    found.is_some()
}

fn open(path: &std::path::Path) -> Result<Session, String> {
    let graph = if path.exists() { Graph::load(path).map_err(|e| e.to_string())? } else { Graph::new() };
    let mut s = Session::new(graph);
    s.path = Some(path.to_path_buf());
    Ok(s)
}

fn save(s: &mut Session) -> Result<(), String> {
    if let Some(dir) = s.path.as_ref().and_then(|p| p.parent()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    s.run("file.save", &json!({})).map(|_| ()).map_err(|e| e.to_string())
}

fn node_arg(s: &mut Session, arg: Option<&String>) -> Result<NodeId, String> {
    match arg.map(String::as_str) {
        None => Ok(s.graph.sys.library),
        Some("today") => s.graph.day(Date::today()).map_err(|e| e.to_string()),
        Some(x) => {
            let id: NodeId = x.parse()?;
            s.graph.node(id).map_err(|e| e.to_string())?;
            Ok(id)
        }
    }
}

/// The built web UI: `web/dist` beside the current folder, else in the source tree.
fn default_web_dir() -> PathBuf {
    let here = PathBuf::from("web/dist");
    if here.join("index.html").exists() { here } else { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist") }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

fn run(mut args: Vec<String>) -> Result<String, String> {
    let explicit_file = args.iter().any(|a| a == "--file");
    let file = take_opt(&mut args, "--file").map(PathBuf::from).unwrap_or_else(tanacraft_core::default_graph_path);
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        return Ok(USAGE.to_string());
    }
    let cmd = args.remove(0);
    let cmd_err = |e: tanacraft_commands::CmdError| e.to_string();
    match cmd.as_str() {
        "inspect" => {
            let mut s = open(&file)?;
            let id = node_arg(&mut s, args.first())?;
            Ok(pretty(&s.run("graph.inspect", &json!({"node": id.0})).map_err(cmd_err)?))
        }
        "view" => {
            let mut s = open(&file)?;
            let id = node_arg(&mut s, args.first())?;
            s.open(id).map_err(cmd_err)?;
            s.run("view.expandAll", &json!({})).map_err(cmd_err)?;
            let v = s.run("graph.view", &json!({})).map_err(cmd_err)?;
            let lines: Vec<&str> = v["lines"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            Ok(format!("{}\n{}", v["title"].as_str().unwrap_or_default(), lines.join("\n")))
        }
        "import" => {
            let parent = take_opt(&mut args, "--parent");
            let path = args.first().ok_or("import needs a FILE")?;
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let mut s = open(&file)?;
            let mut p = json!({"text": text});
            if let Some(par) = parent {
                p["parent"] = json!(node_arg(&mut s, Some(&par))?.0);
            }
            let cmd = if path.to_ascii_lowercase().ends_with(".opml") { "import.opml" } else { "import.markdown" };
            let r = s.run(cmd, &p).map_err(cmd_err)?;
            save(&mut s)?;
            Ok(format!("imported {} nodes", r["created"]))
        }
        "capture" => {
            let tag = take_opt(&mut args, "--tag");
            let name = args.join(" ");
            if name.trim().is_empty() {
                return Err("capture needs TEXT".into());
            }
            let mut s = open(&file)?;
            let mut p = json!({"name": name});
            if let Some(t) = tag {
                p["tag"] = json!(t);
            }
            let r = s.run("node.capture", &p).map_err(cmd_err)?;
            save(&mut s)?;
            Ok(r["id"].to_string())
        }
        "add" => {
            let parent = take_opt(&mut args, "--parent");
            let today = take_flag(&mut args, "--today");
            let mut tags = Vec::new();
            while let Some(t) = take_opt(&mut args, "--tag") {
                tags.push(t);
            }
            let name = args.join(" ");
            if name.trim().is_empty() {
                return Err("add needs a NAME".into());
            }
            let mut s = open(&file)?;
            let parent = if today { node_arg(&mut s, Some(&"today".to_string()))? } else { node_arg(&mut s, parent.as_ref())? };
            let r = s.run("node.add", &json!({"name": name, "parent": parent.0})).map_err(cmd_err)?;
            for t in tags {
                s.run("tag.add", &json!({"node": r["id"], "tag": t})).map_err(cmd_err)?;
            }
            save(&mut s)?;
            Ok(r["id"].to_string())
        }
        "export" => {
            let format = take_opt(&mut args, "--format").unwrap_or_else(|| "md".into());
            let mut s = open(&file)?;
            let id = node_arg(&mut s, args.first())?;
            match format.as_str() {
                "md" | "markdown" => Ok(tanacraft_core::export::to_markdown(&s.graph, id, args.is_empty()).trim_end().to_string()),
                "json" => Ok(pretty(&tanacraft_core::export::to_json_tree(&s.graph, id))),
                // Without a node: the Library's pages; with one: that node and its subtree.
                "opml" => {
                    let ids = if args.is_empty() { s.graph.children(id).to_vec() } else { vec![id] };
                    Ok(tanacraft_core::opml::to_opml(&s.graph, &ids, s.graph.name(id)))
                }
                other => Err(format!("unknown format `{other}` (md, json or opml)")),
            }
        }
        "search" => {
            let mut s = open(&file)?;
            let v = s.run("graph.search", &json!({"query": args.join(" ")})).map_err(cmd_err)?;
            Ok(v.as_array()
                .map(|a| a.iter().map(|h| format!("{}\t{}", h["id"], h["line"].as_str().unwrap_or_default())).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default())
        }
        "tags" => {
            let mut s = open(&file)?;
            Ok(pretty(&s.run("graph.tags", &json!({})).map_err(cmd_err)?))
        }
        "run" => {
            let mut s = open(&file)?;
            let mut out = Vec::new();
            while let Some(c) = take_opt(&mut args, "--cmd") {
                let (id, params) = match c.split_once('=') {
                    Some((id, p)) => (id.to_string(), serde_json::from_str::<Value>(p).map_err(|e| format!("{id}: bad params json: {e}"))?),
                    None => (c, json!({})),
                };
                let r = s.run(&id, &params).map_err(|e| format!("{id}: {e}"))?;
                out.push(format!("{id}: {r}"));
            }
            save(&mut s)?;
            Ok(out.join("\n"))
        }
        "commands" => {
            let s = Session::new(Graph::new());
            Ok(s.registry
                .all()
                .iter()
                .map(|c| format!("{:<22} {:<22} {:<34} {}", c.id, c.shortcut, c.label, c.params))
                .collect::<Vec<_>>()
                .join("\n"))
        }
        "sample" => {
            let out = take_opt(&mut args, "--out").map(PathBuf::from).unwrap_or(file);
            let mut g = sample::sample(Date::today()).map_err(|e| e.to_string())?;
            if let Some(dir) = out.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            g.save(&out).map_err(|e| e.to_string())?;
            Ok(format!("wrote {PRODUCT_NAME} sample graph to {}", out.display()))
        }
        "mcp" => {
            let backend = match take_opt(&mut args, "--connect") {
                Some(addr) => tanacraft_mcp::Backend::connect(&addr)?,
                None => tanacraft_mcp::Backend::Local(Box::new(open(&file)?)),
            };
            let stdin = std::io::stdin();
            tanacraft_mcp::Server::new(backend).serve(stdin.lock(), std::io::stdout().lock()).map_err(|e| e.to_string())?;
            Ok(String::new())
        }
        "serve" => {
            let port = take_opt(&mut args, "--port").map(|p| p.parse::<u16>().map_err(|e| format!("--port: {e}"))).transpose()?.unwrap_or(8787);
            let web = take_opt(&mut args, "--web").map(PathBuf::from).unwrap_or_else(default_web_dir);
            let today = take_opt(&mut args, "--today").map(|d| Date::parse(&d).ok_or(format!("--today: bad date `{d}`"))).transpose()?;
            let sample_mode = take_flag(&mut args, "--sample");
            let session = tanacraft_server::open_session(explicit_file.then_some(file), sample_mode, today)?;
            let shared = tanacraft_server::http::Shared::new(tanacraft_server::Host::new(session));
            eprintln!("{PRODUCT_NAME}: serving http://127.0.0.1:{port}/ (web UI from {})", web.display());
            tanacraft_server::http::serve(shared, port, web)?;
            Ok(String::new())
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    }
}
