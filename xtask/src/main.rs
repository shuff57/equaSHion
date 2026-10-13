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
  web     build the web showcase with trunk (apps/equashion-web -> dist/), print wasm sizes
  serve   serve dist/ at http://127.0.0.1:8780/ (EQUASHION_PORT overrides the port)
  smoke   serve dist/ and run tests/browser/smoke.sh against it (playwright-cli, headless)
";

/// Where `cargo xtask web` puts the site and `serve` / `smoke` read it.
const DIST: &str = "dist";
/// The port `serve` and `smoke` use unless EQUASHION_PORT is set.
const PORT: u16 = 8780;

/// Crates that must build for `wasm32-unknown-unknown`.
const WASM_CRATES: &[(&str, &str)] = &[
    ("equashion-layout", "crates/layout"),
    ("equashion-render", "crates/render"),
    ("equashion-egui", "crates/egui"),
    ("equashion-web", "apps/equashion-web"),
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check") => cmd_check(),
        Some("bless") => cmd_bless(),
        Some("web") => cmd_web(),
        Some("serve") => cmd_serve(),
        Some("smoke") => cmd_smoke(),
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

fn cmd_web() -> Result<(), String> {
    let mut trunk = Command::new("trunk");
    trunk.current_dir(root().join("apps/equashion-web")).args(["build", "--release"]);
    run("trunk build --release", trunk)?;
    let dist = root().join(DIST);
    let mut found = false;
    for e in std::fs::read_dir(&dist).map_err(|e| format!("{}: {e}", dist.display()))?.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "wasm") {
            found = true;
            let raw = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            let gz = Command::new("gzip")
                .args(["-9", "-c"])
                .arg(&p)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| o.stdout.len().to_string())
                .unwrap_or_else(|| "? (no gzip)".into());
            eprintln!("xtask: {}: {raw} bytes raw, {gz} bytes gzip -9", p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
        }
    }
    if !found {
        return Err(format!("no .wasm in {}", dist.display()));
    }
    write_loading_svg(&dist)?;
    eprintln!("xtask: web built into {}", dist.display());
    Ok(())
}

/// The loading splash's art: the motion wordmark with its `prefers-reduced-motion` rule removed, so
/// it always plays. One source (assets/logo/equashion-motion.svg), one derived file in dist/.
fn write_loading_svg(dist: &Path) -> Result<(), String> {
    let src = root().join("assets/logo/equashion-motion.svg");
    let svg = std::fs::read_to_string(&src).map_err(|e| format!("{}: {e}", src.display()))?;
    let start = svg.find("  @media (prefers-reduced-motion: reduce) {").ok_or("no reduced-motion rule in equashion-motion.svg")?;
    let end = start + svg[start..].find("\n  }\n").ok_or("unterminated reduced-motion rule")? + "\n  }\n".len();
    let out = format!("{}{}", &svg[..start], &svg[end..]);
    std::fs::write(dist.join("equashion-loading.svg"), out).map_err(|e| format!("equashion-loading.svg: {e}"))
}

fn port() -> u16 {
    std::env::var("EQUASHION_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(PORT)
}

fn dist_ready() -> Result<PathBuf, String> {
    let dist = root().join(DIST);
    if !dist.join("index.html").is_file() {
        return Err(format!("{}/index.html is missing: run `cargo xtask web` first", dist.display()));
    }
    Ok(dist)
}

fn cmd_serve() -> Result<(), String> {
    let dist = dist_ready()?;
    let listener = serve::bind(port())?;
    eprintln!("xtask: serving {} at http://127.0.0.1:{}/ (Ctrl+C stops)", dist.display(), port());
    serve::run(listener, dist);
    Ok(())
}

fn cmd_smoke() -> Result<(), String> {
    let dist = dist_ready()?;
    let listener = serve::bind(port())?;
    std::thread::spawn(move || serve::run(listener, dist));
    let url = format!("http://127.0.0.1:{}/", port());
    let mut sh = Command::new("bash");
    sh.current_dir(root()).arg("tests/browser/smoke.sh").arg(&url);
    run("smoke", sh)
}

/// A minimal static file server for `dist/` (std only, one thread per connection).
mod serve {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::{Component, Path, PathBuf};

    pub fn bind(port: u16) -> Result<TcpListener, String> {
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("127.0.0.1:{port}: {e}"))
    }

    pub fn run(listener: TcpListener, root: PathBuf) {
        for stream in listener.incoming().flatten() {
            let root = root.clone();
            std::thread::spawn(move || {
                let _ = handle(stream, &root);
            });
        }
    }

    fn mime(p: &Path) -> &'static str {
        match p.extension().and_then(|x| x.to_str()).unwrap_or("") {
            "html" => "text/html; charset=utf-8",
            "js" => "text/javascript; charset=utf-8",
            "wasm" => "application/wasm",
            "css" => "text/css; charset=utf-8",
            "json" => "application/json",
            "png" => "image/png",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            _ => "application/octet-stream",
        }
    }

    /// Biggest accepted report body, bytes (a 1 MB screenshot as base64 plus the text).
    const REPORT_MAX: usize = 2_000_000;

    fn reply(mut out: TcpStream, status: &str, body: &str) -> std::io::Result<()> {
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.len()
        );
        out.write_all(head.as_bytes())?;
        out.write_all(body.as_bytes())?;
        out.flush()
    }

    /// `POST /api/issue-reports` for local runs: the report is written to
    /// `target/issue-reports/<millis>.json` (next to `dist/`'s parent), nothing is sent anywhere.
    /// The deployed site answers this path with `functions/api/issue-reports.js`.
    fn issue_report(reader: &mut BufReader<TcpStream>, out: TcpStream, length: usize, root: &Path) -> std::io::Result<()> {
        if length == 0 || length > REPORT_MAX {
            return reply(out, "400 Bad Request", "{\"error\":\"The report is empty or too large.\"}");
        }
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body)?;
        let text = String::from_utf8_lossy(&body);
        // The same minimum the deployed function enforces; it parses the JSON properly.
        if !text.trim_start().starts_with('{') || !text.contains("\"title\"") || !text.contains("\"description\"") {
            return reply(out, "400 Bad Request", "{\"error\":\"That does not look like a report.\"}");
        }
        let dir = report_dir(root);
        std::fs::create_dir_all(&dir)?;
        let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        std::fs::write(dir.join(format!("{id}.json")), &body)?;
        reply(out, "201 Created", &format!("{{\"success\":true,\"id\":\"{id}\"}}"))
    }

    fn report_dir(root: &Path) -> PathBuf {
        root.parent().unwrap_or(root).join("target").join("issue-reports")
    }

    /// The raw JSON string literal (quotes included) of `"name": "..."` in a body the page wrote.
    /// Raw, so it can be copied into the answer without decoding and re-encoding it.
    fn raw_string<'a>(body: &'a str, name: &str) -> Option<&'a str> {
        let key = format!("\"{name}\":\"");
        let start = body.find(&key)? + key.len() - 1;
        let bytes = body.as_bytes();
        let mut i = start + 1;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += 2,
                b'"' => return body.get(start..=i),
                _ => i += 1,
            }
        }
        None
    }

    /// The votes file: one `id|address` line per vote, so one address votes once per report.
    fn votes(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("votes.txt")).unwrap_or_default().lines().map(str::to_string).collect()
    }

    /// `GET /api/issue-reports`: the reports written so far, most voted first, in the shape the
    /// deployed function answers with (no context, no screenshot).
    fn issue_list(out: TcpStream, root: &Path) -> std::io::Result<()> {
        let dir = report_dir(root);
        let votes = votes(&dir);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect())
            .unwrap_or_default();
        files.sort();
        files.reverse();
        let mut rows: Vec<(usize, String)> = Vec::new();
        for f in files {
            let (Some(id), Ok(body)) = (f.file_stem().and_then(|s| s.to_str()), std::fs::read_to_string(&f)) else { continue };
            let (Some(title), Some(desc)) = (raw_string(&body, "title"), raw_string(&body, "description")) else { continue };
            let kind = raw_string(&body, "kind").unwrap_or("\"bug\"");
            let n = votes.iter().filter(|v| v.split('|').next() == Some(id)).count();
            rows.push((
                n,
                format!("{{\"id\":\"{id}\",\"kind\":{kind},\"title\":{title},\"description\":{desc},\"status\":\"open\",\"votes\":{n},\"createdAt\":null,\"screenshotUrl\":null}}"),
            ));
        }
        rows.sort_by_key(|r| std::cmp::Reverse(r.0));
        let list: Vec<String> = rows.into_iter().map(|r| r.1).collect();
        reply(out, "200 OK", &format!("{{\"reports\":[{}]}}", list.join(",")))
    }

    /// `POST /api/issue-reports/vote {"id": "..."}`: one vote per address per report.
    fn issue_vote(reader: &mut BufReader<TcpStream>, out: TcpStream, length: usize, root: &Path, peer: &str) -> std::io::Result<()> {
        if length == 0 || length > 1000 {
            return reply(out, "400 Bad Request", "{\"error\":\"That is not a valid vote.\"}");
        }
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body)?;
        let text = String::from_utf8_lossy(&body).into_owned();
        let id = raw_string(&text, "id").map(|s| s.trim_matches('"').to_string()).unwrap_or_default();
        let dir = report_dir(root);
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) || !dir.join(format!("{id}.json")).is_file() {
            return reply(out, "400 Bad Request", "{\"error\":\"Unknown issue.\"}");
        }
        let mut all = votes(&dir);
        let line = format!("{id}|{peer}");
        if !all.contains(&line) {
            all.push(line);
            std::fs::write(dir.join("votes.txt"), all.join("\n") + "\n")?;
        }
        let n = all.iter().filter(|v| v.split('|').next() == Some(id.as_str())).count();
        reply(out, "200 OK", &format!("{{\"votes\":{n}}}"))
    }

    fn handle(stream: TcpStream, root: &Path) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        // Drain the headers, keeping the body length.
        let mut h = String::new();
        let mut length = 0usize;
        while reader.read_line(&mut h)? > 2 {
            if let Some((k, v)) = h.split_once(':')
                && k.eq_ignore_ascii_case("content-length")
            {
                length = v.trim().parse().unwrap_or(0);
            }
            h.clear();
        }
        let mut parts = line.split_whitespace();
        let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
        let path = target.split(['?', '#']).next().unwrap_or("/");
        let peer = stream.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
        match (method, path) {
            ("POST", "/api/issue-reports") => return issue_report(&mut reader, stream, length, root),
            ("POST", "/api/issue-reports/vote") => return issue_vote(&mut reader, stream, length, root, &peer),
            ("GET", "/api/issue-reports") => return issue_list(stream, root),
            _ => {}
        }
        let rel = Path::new(path.trim_start_matches('/'));
        let safe = rel.components().all(|c| matches!(c, Component::Normal(_)));
        let mut file = root.join(rel);
        if file.is_dir() {
            file = file.join("index.html");
        }
        let mut out = stream;
        match (method, safe, std::fs::read(&file)) {
            ("GET" | "HEAD", true, Ok(body)) => {
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                    mime(&file),
                    body.len()
                );
                out.write_all(head.as_bytes())?;
                if method == "GET" {
                    out.write_all(&body)?;
                }
            }
            _ => {
                out.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found")?;
            }
        }
        out.flush()
    }
}
