//! 页面文件：只认 `GET`，路径落在页面目录里才给；别的 404。开发用，不缓存。`/file`、`/blob` 交给 `media.rs`，`/pick-dir` 交给 `dialog.rs`，
//! 软件后台页的 `POST /page`、`/p/…` 交给 `pages.rs`。

use std::io;
use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{Site, media};

/// 读一个请求，回一个文件。
///
/// # Errors
///
/// 读写连接出错。
pub async fn serve(mut stream: TcpStream, site: &Site) -> io::Result<()> {
    let dir = site.dir.as_path();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    // 请求头到空行为止
    let end = buf.windows(4).position(|w| w == b"\r\n\r\n").map_or(buf.len(), |i| i + 4);
    let head = String::from_utf8_lossy(&buf[..end]);
    let mut parts = head.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let query = target.split_once('?').map_or("", |(_, q)| q.split('#').next().unwrap_or(""));
    if method == "POST" && path == "/page" {
        let body = body(&mut stream, &buf[end..], &head).await?;
        return crate::pages::issue(&mut stream, site, &head, &body).await;
    }
    if method != "GET" {
        return reply(&mut stream, "405 Method Not Allowed", "text/plain; charset=utf-8", b"only GET").await;
    }
    if path.starts_with("/p/") {
        return crate::pages::serve(&mut stream, site, path).await;
    }
    if path == "/file" || path == "/blob" {
        return media::serve(&mut stream, site, path, query, &head).await;
    }
    if path == "/pick-dir" {
        // 选目录（`dialog.rs`）：带口令才开；`start` 是从哪个目录开始，`title` 是对话框的标题
        let q = media::params(query);
        if q.get("k") != Some(&site.key) {
            return reply(&mut stream, "403 Forbidden", "text/plain; charset=utf-8", b"").await;
        }
        let start = q.get("start").filter(|s| !s.is_empty()).map(String::as_str);
        let picked = crate::dialog::pick_dir(q.get("title").map_or("", String::as_str), start).await;
        return reply(&mut stream, "200 OK", "application/json; charset=utf-8", picked.json().as_bytes()).await;
    }
    if path == "/key" {
        // 页面连不上时问一句口令对不对（蓝图 `web.md`「连核心」第 9 条）：只回对不对，别的不说
        let ok = media::params(query).get("k") == Some(&site.key);
        return reply(&mut stream, if ok { "204 No Content" } else { "403 Forbidden" }, "text/plain; charset=utf-8", b"").await;
    }
    let path = decode(if path == "/" { "/index.html" } else { path });
    let Some(file) = resolve(dir, &path) else {
        return reply(&mut stream, "404 Not Found", "text/plain; charset=utf-8", b"not found").await;
    };
    match tokio::fs::read(&file).await {
        Ok(body) => reply(&mut stream, "200 OK", kind(&file), &body).await,
        Err(_) => reply(&mut stream, "404 Not Found", "text/plain; charset=utf-8", b"not found").await,
    }
}

/// 正文最多多大（`POST /page` 只有一个包的编号）。
const BODY_MAX: usize = 16 * 1024;

/// 读请求的正文：照 `Content-Length`，已经读进来的接着读，最多 `BODY_MAX`。
async fn body(stream: &mut TcpStream, got: &[u8], head: &str) -> io::Result<Vec<u8>> {
    let len = head.lines().find_map(|l| {
        let (name, value) = l.split_once(':')?;
        name.trim().eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().ok()).flatten()
    }).unwrap_or(0).min(BODY_MAX);
    let mut body = got[..got.len().min(len)].to_vec();
    let mut chunk = [0u8; 2048];
    while body.len() < len {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n.min(len - body.len())]);
    }
    Ok(body)
}

/// 路径换成页面目录里的文件：带 `..` 的、跑到目录外的、不是文件的都不给。
fn resolve(dir: &Path, path: &str) -> Option<std::path::PathBuf> {
    // 桥自己的源码和编出来的东西不给
    if path.split('/').any(|seg| seg == "..") || path.trim_start_matches('/').starts_with("bridge/") {
        return None;
    }
    let file = dir.join(path.trim_start_matches('/')).canonicalize().ok()?;
    (file.starts_with(dir) && file.is_file()).then_some(file)
}

/// `%xx` 换回字节（中文文件名、空格）。
pub(crate) fn decode(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push((a * 16 + b) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn kind(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("md") => "text/markdown; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub(crate) async fn reply(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.shutdown().await
}
