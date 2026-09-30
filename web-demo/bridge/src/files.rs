//! 页面文件：只认 `GET`，路径落在页面目录里才给；别的 404。开发用，不缓存。`/file`、`/blob` 交给 `media.rs`，
//! `/link-image` 交给 `link_preview/`，`POST /upload` 交给 `upload.rs`。

use std::io;
use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{Site, link_preview, media, upload};

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
    // 请求头到空行为止；后面跟着读进来的是正文的开头（`POST /upload` 的）
    let end = buf.windows(4).position(|w| w == b"\r\n\r\n").map_or(buf.len(), |i| i + 4);
    let head = String::from_utf8_lossy(&buf[..end]);
    let mut parts = head.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let query = target.split_once('?').map_or("", |(_, q)| q.split('#').next().unwrap_or(""));
    if method == "POST" && path == "/upload" {
        return upload::serve(&mut stream, site, query, &head, &buf[end..]).await;
    }
    if method != "GET" {
        return reply(&mut stream, "405 Method Not Allowed", "text/plain; charset=utf-8", b"only GET").await;
    }
    if path == "/file" || path == "/blob" {
        return media::serve(&mut stream, site, path, query, &head).await;
    }
    if path == "/link-image" {
        return link_preview::serve(&mut stream, site, query).await;
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
