//! Workspace tooling: `cargo xtask <command>`. Pure std; external tools run via `Command`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  ci      fmt --check, clippy -D warnings, test (stops at the first failure)
  shots   render the README screenshots offscreen into docs/images/
";

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("ci") => ci(),
        Some("shots") => shots(),
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

fn shots() -> Result<(), String> {
    let script = root().join("docs/shots.jsonl");
    cargo(&["run", "-p", "tanacraft-ui-egui", "--example", "ui_shot", "--", &script.to_string_lossy()])
}
