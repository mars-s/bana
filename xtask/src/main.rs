//! Workspace tooling: `cargo xtask <command>`. Pure std; external tools run via `Command`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  ci      fmt --check, clippy -D warnings, test (stops at the first failure)
  web     build the web UI and run its Playwright tests (needs pnpm)
  shots   web UI screenshots (Playwright) into docs/images/web/
  connector  the remote MCP connector's tests (cloudflare/: npm ci, typecheck, Worker + end to end)
  sync    the sync checks: the Rust sync tests, the relay Worker's tests (cloudflare/sync, vitest
          in workerd) and its typecheck
  parity  check that docs/parity.md maps every spec file and section; print status counts
";

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("ci") => ci(),
        // The Playwright tests start `target/debug/tanacraft-cli serve`, so it is built first.
        Some("shots") => web_prep().and_then(|_| pnpm(&["run", "shots"])),
        Some("web") => web_prep().and_then(|_| pnpm(&["exec", "playwright", "test", "--project=tests"])),
        Some("parity") => parity(),
        Some("sync") => sync(),
        Some("connector") => cargo(&["build", "-p", "tanacraft-cli"])
            .and_then(|_| npm(&["ci", "--no-audit", "--no-fund"]))
            .and_then(|_| npm(&["run", "typecheck"]))
            .and_then(|_| npm(&["test"])),
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
    // The iPhone app's tokens are generated from web/src/tokens.css: they must not drift.
    let tokens =
        Command::new("python3").arg(root().join("scripts/gen-ios-tokens.py")).arg("--check").status().map_err(|e| format!("python3: {e}"))?;
    if !tokens.success() {
        return Err("apps/tanacraft-ios/TanaCraft/Tokens.swift is out of date: run python3 scripts/gen-ios-tokens.py".into());
    }
    cargo(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])?;
    cargo(&["test", "--workspace"])
}

fn sync() -> Result<(), String> {
    cargo(&["test", "-p", "tanacraft-sync"])?;
    let relay = root().join("cloudflare/sync");
    run_in(&relay, "npm", &["ci", "--no-audit", "--no-fund"])?;
    run_in(&relay, "npm", &["run", "typecheck"])?;
    run_in(&relay, "npm", &["test"])
}

fn run_in(dir: &Path, cmd: &str, args: &[&str]) -> Result<(), String> {
    let what = format!("{cmd} {}", args.join(" "));
    eprintln!("$ {what} (in {})", dir.display());
    let status = Command::new(cmd).args(args).current_dir(dir).status().map_err(|e| format!("{what}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{what}: exited with {status}")) }
}

fn web_prep() -> Result<(), String> {
    cargo(&["build", "-p", "tanacraft-cli"])?;
    pnpm(&["install", "--frozen-lockfile"])?;
    pnpm(&["run", "build"])
}

fn pnpm(args: &[&str]) -> Result<(), String> {
    let what = format!("pnpm {}", args.join(" "));
    eprintln!("$ {what} (in web/)");
    let status = Command::new("pnpm").args(args).current_dir(root().join("web")).status().map_err(|e| format!("{what}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{what}: exited with {status}")) }
}

fn npm(args: &[&str]) -> Result<(), String> {
    let what = format!("npm {}", args.join(" "));
    eprintln!("$ {what} (in cloudflare/)");
    let status = Command::new("npm").args(args).current_dir(root().join("cloudflare")).status().map_err(|e| format!("{what}: {e}"))?;
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
