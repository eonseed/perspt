//! Live preview server (`cargo xtask serve`) for the assembled Guidedog site.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) fn run(root: &Path, port: u16, no_pdf: bool, guidedog_bin: &str) -> Result<()> {
    let guidedog = crate::docs::resolve_guidedog(guidedog_bin)?;
    let site = root.join("docs/_build/site");
    rebuild(root, &guidedog, &site, no_pdf);

    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("binding 127.0.0.1:{port}"))?;
    println!(
        "Serving {} at http://127.0.0.1:{port}/ (Ctrl+C to stop)",
        site.display()
    );

    let watched_root = root.to_path_buf();
    let watched_site = site.clone();
    std::thread::spawn(move || watch(&watched_root, &guidedog, &watched_site, no_pdf));

    for stream in listener.incoming().flatten() {
        if let Err(err) = respond(stream, &site) {
            eprintln!("Request failed: {err}");
        }
    }
    Ok(())
}

fn rebuild(root: &Path, guidedog: &Path, site: &Path, no_pdf: bool) {
    let mut cmd = Command::new(guidedog);
    cmd.current_dir(root)
        .args(["ssg", "build", "docs", "--out"])
        .arg(site);
    if no_pdf {
        cmd.arg("--no-pdf");
    }
    match cmd.status() {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!("Site build failed ({status}); serving previous output."),
        Err(err) => eprintln!("Cannot run Guidedog: {err}"),
    }
}

fn watch(root: &Path, guidedog: &Path, site: &Path, no_pdf: bool) {
    let mut seen = newest_mtime(root);
    loop {
        std::thread::sleep(POLL_INTERVAL);
        let now = newest_mtime(root);
        if now != seen {
            println!("Documentation sources changed; rebuilding site...");
            rebuild(root, guidedog, site, no_pdf);
            seen = newest_mtime(root);
        }
    }
}

fn newest_mtime(root: &Path) -> (Option<SystemTime>, usize) {
    let mut stack = vec![root.join("docs")];
    let (mut latest, mut count) = (None, 0);
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if entry.file_name() != "_build" {
                    stack.push(entry.path());
                }
            } else if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                latest = latest.max(Some(modified));
                count += 1;
            }
        }
    }
    (latest, count)
}

fn respond(mut stream: TcpStream, site: &Path) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    let mut header = String::new();
    while reader.read_line(&mut header)? > 2 {
        header.clear();
    }
    let target = request.split_whitespace().nth(1).unwrap_or("/");
    let (status, mime, body) =
        match resolve_path(site, target).and_then(|p| std::fs::read(&p).ok().map(|b| (p, b))) {
            Some((path, body)) => ("200 OK", content_type(&path), body),
            None => ("404 Not Found", "text/plain", b"Not found\n".to_vec()),
        };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)
}

fn resolve_path(site: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let relative = PathBuf::from(decode_percent(path)?.trim_start_matches('/'));
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let full = site.join(relative);
    if full.is_dir() {
        Some(full.join("index.html"))
    } else {
        Some(full)
    }
}

fn decode_percent(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'%' {
            let hex = std::str::from_utf8(bytes.get(idx + 1..idx + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            idx += 3;
        } else {
            decoded.push(bytes[idx]);
            idx += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "pdf" => "application/pdf",
        "woff2" => "font/woff2",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_resolution_rejects_traversal() {
        let site = Path::new("/site");
        assert_eq!(
            resolve_path(site, "/book/index.html?x=1"),
            Some(PathBuf::from("/site/book/index.html"))
        );
        assert_eq!(resolve_path(site, "/../etc/passwd"), None);
        assert_eq!(resolve_path(site, "/%2e%2e/x"), None);
    }
}
