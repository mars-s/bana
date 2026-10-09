//! Workspace tooling: `cargo xtask <command>`. Pure std; external tools run via `Command`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  ci      fmt --check, clippy -D warnings, test (stops at the first failure)
  web     build the web UI and run its Playwright tests (needs pnpm)
  shots   web UI screenshots (Playwright) into docs/images/web/
  parity  check that docs/parity.md maps every spec file and section; print status counts
";

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("ci") => ci(),
        Some("shots") => pnpm(&["run", "shots"]),
        Some("web") => pnpm(&["install", "--frozen-lockfile"]).and_then(|_| pnpm(&["run", "build"])).and_then(|_| pnpm(&["test"])),
        Some("parity") => parity(),
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}

fn cargo(args: &[&str]) -> Result<(), String> {
    let what = format!("cargo {}", args.join(" "));
    eprintln!("$ {what}");
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(args)
        .current_dir(root())
        .status()
        .map_err(|e| format!("{what}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{what}: exited with {status}")) }
}

fn ci() -> Result<(), String> {
    cargo(&["fmt", "--all", "--check"])?;
    cargo(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    cargo(&["test", "--workspace"])
}

fn pnpm(args: &[&str]) -> Result<(), String> {
    let what = format!("pnpm {}", args.join(" "));
    eprintln!("$ {what} (in web/)");
    let status = Command::new("pnpm").args(args).current_dir(root().join("web")).status().map_err(|e| format!("{what}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{what}: exited with {status}")) }
}

/// Every screenshot and every `##` section of the spec must appear in docs/parity.md.
fn parity() -> Result<(), String> {
    let spec = root().join("../tanacraft-spec");
    let doc = std::fs::read_to_string(root().join("docs/parity.md")).map_err(|e| format!("docs/parity.md: {e}"))?;
    let mut items = Vec::new();
    let mut stack = vec![spec.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "DIFFS.md" || name == "diffs" {
                continue; // tool metadata and the acceptance-test report, not spec
            }
            if p.is_dir() {
                stack.push(p);
            } else if name.ends_with(".md") {
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                items.push(name.clone());
                items.extend(text.lines().filter_map(|l| l.strip_prefix("## ")).map(|h| format!("{name}: {}", h.trim())));
            } else if !name.starts_with('.') {
                items.push(name);
            }
        }
    }
    items.sort();
    // A section counts as mapped when its heading text appears; a file when its name does.
    let missing: Vec<&String> = items.iter().filter(|i| !doc.contains(i.rsplit(": ").next().unwrap_or(i))).collect();
    let count = |s: &str| doc.lines().filter(|l| l.starts_with("| ") && l.contains(&format!(" | {s} | "))).count();
    println!(
        "spec items: {}  rows done: {}  partial: {}  missing: {}  n/a: {}",
        items.len(),
        count("done"),
        count("partial"),
        count("missing"),
        count("n/a")
    );
    if missing.is_empty() {
        return Ok(());
    }
    for m in &missing {
        println!("not in docs/parity.md: {m}");
    }
    Err(format!("{} spec items are not mapped", missing.len()))
}
