//! Workspace tooling: `cargo xtask <command>` (alias in `.cargo/config.toml`).
//!
//! Pure std. `cargo` is invoked through `std::process::Command`.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  check   cargo fmt --check, clippy -D warnings, test, and a wasm32 check of the
          web-facing crates (stops at the first failure)
  bless   rewrite the golden files (EQUASHION_BLESS=1 cargo test -p equashion-render)
";

/// Crates that must build for `wasm32-unknown-unknown` (the web page comes next).
const WASM_CRATES: &[(&str, &str)] = &[("equashion-layout", "crates/layout"), ("equashion-render", "crates/render")];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check") => cmd_check(),
        Some("bless") => cmd_bless(),
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The workspace root (the parent of this crate's directory).
fn root() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.parent().map(Path::to_path_buf).unwrap_or(here)
}

fn cargo() -> Command {
    let mut c = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    c.current_dir(root());
    c
}

fn run(step: &str, mut cmd: Command) -> Result<(), String> {
    eprintln!("xtask: == {step} ==");
    let status = cmd.status().map_err(|e| format!("{step}: could not start: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{step} failed ({status})")) }
}

fn cmd_check() -> Result<(), String> {
    let mut fmt = cargo();
    fmt.args(["fmt", "--all", "--check"]);
    run("fmt", fmt)?;

    let mut clippy = cargo();
    clippy.args(["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]);
    run("clippy", clippy)?;

    let mut test = cargo();
    test.args(["test", "--workspace"]);
    run("test", test)?;

    let present: Vec<&str> = WASM_CRATES.iter().filter(|(_, dir)| root().join(dir).join("Cargo.toml").is_file()).map(|(name, _)| *name).collect();
    if present.is_empty() {
        eprintln!("xtask: == wasm == skipped (no web-facing crates yet)");
    } else {
        let mut wasm = cargo();
        wasm.args(["check", "--target", "wasm32-unknown-unknown"]);
        for p in &present {
            wasm.args(["-p", p]);
        }
        run("wasm", wasm)?;
    }
    eprintln!("xtask: check passed");
    Ok(())
}

fn cmd_bless() -> Result<(), String> {
    let mut test = cargo();
    test.env("EQUASHION_BLESS", "1").args(["test", "-p", "equashion-render"]);
    run("bless", test)
}
