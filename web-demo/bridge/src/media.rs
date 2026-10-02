//! 本机文件和 blob 经 HTTP 给页面（蓝图 `web.md`「读本机文件」）：`<img>`、`<video>`、`<audio>` 要一个地址，音视频还要拖进度。
//!
//! 读的那一半归核心（核心施工 W-6，2026-10-02 起）：桥不自己读盘、不读会话日志，连一条核心连接，照要的那一段一块块问
//! `fs.read`、`blob.get`（一块最多 512 KiB），边问边写给浏览器，不整个读进内存。能读什么照核心的规矩。
//!
//! - 口令：地址里的 `k` 和桥的访问口令对上才给（和 `/ws` 同一个）。
//! - `/file?k=…&path=<绝对路径>`：经 `fs.read`；数据根不给（工作区里的能读）、一层链接都不跟，核心定。
//! - `/blob?k=…&hash=sha256:…&type=<媒体类型>`：经 `blob.get`，照这个账号的 blob 给；`type` 只认 `media.json` 列的几种。
//! - `Range: bytes=a-b` 只认一段，回 206；超出的回 416。`download=1` 的叫浏览器存下来（带文件名，`name` 给了的照它）。
//! - 回的都带 `nosniff`、不缓存，再加一条 `sandbox` 的内容安全策略：有人直接打开这个地址（一个 SVG、一个 HTML），它在一个空的
//!   来源里跑，碰不到页面、拿不到口令。

use std::collections::HashMap;
use std::io;
use std::path::Path;

use base64::Engine as _;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::core::{Core, Refused};
use crate::{Site, files::decode};

/// 一块问核心要多少字节：核心给的上限（`fs.read`、`blob.get` 的 `length` 最多 512 KiB）。
const CHUNK: u64 = 512 * 1024;

/// 媒体类型的表（`resources/media.json`）。
pub struct Types {
    by_ext: HashMap<String, String>,
    text: String,
    blob: Vec<String>,
}

impl Types {
    /// 读页面目录下的 `resources/media.json`。
    ///
    /// # Errors
    ///
    /// 读不到、不是 JSON、少了哪一格：说是哪一样。
    pub fn load(dir: &Path) -> Result<Types, String> {
        let file = dir.join("resources/media.json");
        let text = std::fs::read_to_string(&file).map_err(|e| format!("读不了 {}：{e}", file.display()))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{} 不是 JSON：{e}", file.display()))?;
        let bad = |k: &str| format!("{} 少了 {k}", file.display());
        let by_ext = v["types"].as_object().ok_or_else(|| bad("types"))?
            .iter().filter_map(|(k, t)| Some((k.clone(), t.as_str()?.to_string()))).collect();
        let blob = v["blob_types"].as_array().ok_or_else(|| bad("blob_types"))?
            .iter().filter_map(|t| t.as_str().map(str::to_string)).collect();
        Ok(Types { by_ext, text: v["text"].as_str().ok_or_else(|| bad("text"))?.to_string(), blob })
    }

    /// 照扩展名：表里没有的当 UTF-8 文字。
    pub(crate) fn of(&self, path: &Path) -> String {
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
        self.by_ext.get(&ext).cloned().unwrap_or_else(|| self.text.clone())
    }
}

/// 地址里 `?` 后面的几项，`%xx`、`+` 换回来。
pub(crate) fn params(query: &str) -> HashMap<String, String> {
    query.split('&').filter_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        Some((decode(k), decode(&v.replace('+', " "))))
    }).collect()
}

/// 回一个文件或一个 blob；认不了的回 4xx 和一句为什么。
///
/// # Errors
///
/// 读写连接、读文件出错。
pub async fn serve(stream: &mut TcpStream, site: &Site, route: &str, query: &str, head: &str) -> io::Result<()> {
    let q = params(query);
    let (source, kind) = match locate(site, route, &q) {
        Ok(found) => found,
        Err((status, why)) => return plain(stream, status, &why).await,
    };
    // 要存下来的：带上文件名（UTF-8 照 RFC 5987 转义）；给了 `name` 的（附件原来的名字）照它，只留最后一段，没给的照路径、哈希
    let named = q.get("name").and_then(|n| safe_name(n));
    let save = (q.get("download").map(String::as_str) == Some("1"))
        .then(|| named.or_else(|| safe_name(source.name())))
        .flatten()
        .map(|n| n.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>());
    let mut core = match Core::open().await {
        Ok(core) => core,
        Err(why) => return plain(stream, "502 Bad Gateway", &why).await,
    };
    send(stream, &mut core, &source, &kind, range(head), save.as_deref()).await
}

/// 文件名最长多少字节（常见文件系统的上限）。
const NAME_MAX: usize = 255;

/// 存下来叫什么只留最后一段：不带路径、不是 `.`、`..`、不含控制字符、不超长；不合的是 `None`。
fn safe_name(name: &str) -> Option<String> {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if last.is_empty() || last == "." || last == ".." || last.len() > NAME_MAX || last.chars().any(char::is_control) {
        return None;
    }
    Some(last.to_string())
}

/// 读哪一样：本机的一个路径（`fs.read`），或者一个 blob 哈希（`blob.get`）。
enum Source {
    File(String),
    Blob(String),
}

impl Source {
    /// 存下来时默认叫什么：路径、哈希。
    fn name(&self) -> &str {
        match self {
            Self::File(path) | Self::Blob(path) => path,
        }
    }

    /// 问核心要从 `offset` 起的 `length` 个字节（`0` 只问大小）：交回这一段和一共几个字节。
    async fn read(&self, core: &mut Core, offset: u64, length: u64) -> Result<(Vec<u8>, u64), Refused> {
        let (method, params) = match self {
            Self::File(path) => ("fs.read", json!({"path": path, "offset": offset, "length": length})),
            Self::Blob(hash) => ("blob.get", json!({"blob": hash, "offset": offset, "length": length})),
        };
        let got = core.call(method, params).await?;
        let data = base64::engine::general_purpose::STANDARD
            .decode(got["data"].as_str().unwrap_or(""))
            .map_err(|e| Refused { reason: "bridge".to_string(), message: format!("核心给的不是 base64：{e}") })?;
        Ok((data, got["size"].as_u64().unwrap_or(0)))
    }
}

/// 核心拒的原因码换成 HTTP 的状态。
fn status_of(reason: &str) -> &'static str {
    match reason {
        "path_forbidden" => "403 Forbidden",
        "path_unreadable" | "unknown_blob" => "404 Not Found",
        "bad_params" => "400 Bad Request",
        _ => "502 Bad Gateway",
    }
}

/// 照地址认出读哪一样和它的媒体类型；不给的交回状态和为什么。
fn locate(site: &Site, route: &str, q: &HashMap<String, String>) -> Result<(Source, String), (&'static str, String)> {
    if q.get("k") != Some(&site.key) {
        return Err(("403 Forbidden", "口令不对".to_string()));
    }
    match route {
        "/file" => {
            let path = q.get("path").cloned().unwrap_or_default();
            if !Path::new(&path).is_absolute() {
                return Err(("400 Bad Request", "要绝对路径".to_string()));
            }
            let kind = site.types.of(Path::new(&path));
            Ok((Source::File(path), kind))
        }
        "/blob" => {
            let hash = q.get("hash").cloned().unwrap_or_default();
            let kind = q.get("type").filter(|t| site.types.blob.contains(t)).cloned().unwrap_or_else(|| "application/octet-stream".to_string());
            Ok((Source::Blob(hash), kind))
        }
        _ => Err(("404 Not Found", "没有这个地址".to_string())),
    }
}

/// 请求头里的 `Range: bytes=…`（只认一段）：`(开头, 结尾)`，结尾含在内、可以没有；`bytes=-N` 是最后 N 个字节。
fn range(head: &str) -> Option<(Option<u64>, Option<u64>)> {
    let line = head.lines().find(|l| l.to_ascii_lowercase().starts_with("range:"))?;
    let spec = line.split_once(':')?.1.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    Some((a.trim().parse().ok(), b.trim().parse().ok()))
}

/// 回哪一段：没有 Range 的是全部（200）；有的是那一段（206），超出的是 `None`（416）。交回状态、开头、长度。
fn span(range: Option<(Option<u64>, Option<u64>)>, total: u64) -> Option<(&'static str, u64, u64)> {
    let Some((a, b)) = range else { return Some(("200 OK", 0, total)) };
    let last = total.checked_sub(1)?;
    let (start, end) = match (a, b) {
        (Some(a), Some(b)) => (a, b.min(last)),
        (Some(a), None) => (a, last),
        (None, Some(n)) => (total.saturating_sub(n), last),
        (None, None) => (0, last),
    };
    (start <= end).then(|| ("206 Partial Content", start, end - start + 1))
}

/// 回一个文件、一个 blob：先问一共多大，再照那一段一块块问核心、一块块写给浏览器。
async fn send(stream: &mut TcpStream, core: &mut Core, source: &Source, kind: &str, range: Option<(Option<u64>, Option<u64>)>, save: Option<&str>) -> io::Result<()> {
    let total = match source.read(core, 0, 0).await {
        Ok((_, size)) => size,
        Err(refused) => return plain(stream, status_of(&refused.reason), &refused.message).await,
    };
    let Some((status, start, len)) = span(range, total) else {
        let head = format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return stream.write_all(head.as_bytes()).await;
    };
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {len}\r\nAccept-Ranges: bytes\r\n\
         X-Content-Type-Options: nosniff\r\nCache-Control: private, no-cache\r\n\
         Content-Security-Policy: sandbox; default-src 'none'; img-src data:; media-src data:; style-src 'unsafe-inline'\r\n\
         Connection: close\r\n"
    );
    if let Some(name) = save {
        head.push_str(&format!("Content-Disposition: attachment; filename*=UTF-8''{name}\r\n"));
    }
    if status.starts_with("206") {
        head.push_str(&format!("Content-Range: bytes {start}-{}/{total}\r\n", start + len - 1));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await?;
    let mut at = start;
    while at < start + len {
        // 头已经写出去了：中途出错（文件变了、核心断了）只能断开，浏览器照长度对不上认出来
        let Ok((data, _)) = source.read(core, at, CHUNK.min(start + len - at)).await else { return Ok(()) };
        if data.is_empty() {
            return Ok(());
        }
        stream.write_all(&data).await?;
        at += data.len() as u64;
    }
    Ok(())
}

/// 回一句纯文字（出错的时候）。
///
/// # Errors
///
/// 写连接出错。
pub(crate) async fn plain(stream: &mut TcpStream, status: &str, text: &str) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        text.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(text.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_keep_only_the_last_part() {
        assert_eq!(safe_name("报告.pdf").as_deref(), Some("报告.pdf"));
        assert_eq!(safe_name("a/b/c.png").as_deref(), Some("c.png"));
        assert_eq!(safe_name("..\\x.txt").as_deref(), Some("x.txt"));
        assert_eq!(safe_name(".."), None);
        assert_eq!(safe_name(""), None);
        assert_eq!(safe_name("a\nb"), None);
        assert_eq!(safe_name(&"x".repeat(300)), None);
    }

    #[test]
    fn spans_follow_the_range() {
        assert_eq!(span(None, 10), Some(("200 OK", 0, 10)));
        assert_eq!(span(Some((Some(2), Some(5))), 10), Some(("206 Partial Content", 2, 4)));
        assert_eq!(span(Some((Some(2), None)), 10), Some(("206 Partial Content", 2, 8)));
        assert_eq!(span(Some((None, Some(3))), 10), Some(("206 Partial Content", 7, 3)), "最后 3 个");
        assert_eq!(span(Some((Some(4), Some(99))), 10), Some(("206 Partial Content", 4, 6)), "结尾过了照到头");
        assert_eq!(span(Some((Some(10), None)), 10), None, "开头过了结尾：416");
        assert_eq!(span(Some((Some(0), None)), 0), None, "空的不认 Range");
        assert_eq!(span(None, 0), Some(("200 OK", 0, 0)));
    }

    #[test]
    fn refusals_become_statuses() {
        assert_eq!(status_of("path_forbidden"), "403 Forbidden");
        assert_eq!(status_of("path_unreadable"), "404 Not Found");
        assert_eq!(status_of("unknown_blob"), "404 Not Found");
        assert_eq!(status_of("bad_params"), "400 Bad Request");
        assert_eq!(status_of("bridge"), "502 Bad Gateway");
    }
}
