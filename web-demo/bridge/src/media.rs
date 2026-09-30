//! 本机文件和 blob 经 HTTP 给页面（蓝图 `web.md`「读本机文件」）：`<img>`、`<video>`、`<audio>` 要一个地址，音视频还要拖进度。
//!
//! 顶替核心以后给的：小的经协议（`blob.get`、读文件的查询），大的、要 Range 的由网页模块（设计 21 X5）给带登录令牌的地址。
//! 形状照那个做（按会话、按路径或哈希取），搬的时候只换一层（2026-09-30 和核心施工的会话定）。
//!
//! - 口令：地址里的 `k` 和桥的访问口令对上才给（和 `/ws` 同一个）。
//! - `/file?k=…&session=…&path=<绝对路径>`：整盘能读，数据根不给（`11-权限与沙盒.md` 第五节，核心现在读的规矩）。认真实位置：
//!   链接、`..` 换掉以后再比；只给普通文件。
//! - `/blob?k=…&session=…&hash=sha256:…&type=<媒体类型>`：只给这个会话日志里出现过的哈希；`type` 只认 `media.json` 列的几种。
//! - `Range: bytes=a-b` 只认一段，回 206；超出的回 416。`download=1` 的叫浏览器存下来（带文件名，`name` 给了的照它）。
//! - 回的都带 `nosniff`、不缓存，再加一条 `sandbox` 的内容安全策略：有人直接打开这个地址（一个 SVG、一个 HTML），它在一个空的
//!   来源里跑，碰不到页面、拿不到口令。

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::TcpStream;

use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::{Site, files::decode, history, upload};

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
    let found = match locate(site, route, &q) {
        Ok(found) => found,
        Err((status, why)) => return plain(stream, status, &why).await,
    };
    let (file, kind) = found;
    // 要存下来的：带上文件名（UTF-8 照 RFC 5987 转义）；给了 `name` 的（附件原来的名字）照它，只留最后一段
    let named = q.get("name").and_then(|n| upload::safe_name(n));
    let save = (q.get("download").map(String::as_str) == Some("1"))
        .then(|| named.or_else(|| file.file_name().map(|n| n.to_string_lossy().into_owned())))
        .flatten()
        .map(|n| n.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>());
    send(stream, &file, &kind, range(head), save.as_deref()).await
}

/// 照地址找到文件和它的媒体类型；不给的交回状态和为什么。
fn locate(site: &Site, route: &str, q: &HashMap<String, String>) -> Result<(PathBuf, String), (&'static str, String)> {
    if q.get("k") != Some(&site.key) {
        return Err(("403 Forbidden", "口令不对".to_string()));
    }
    let account = site.account.lock().ok().and_then(|a| a.clone()).ok_or(("403 Forbidden", "页面还没握手".to_string()))?;
    let root = DataRoot::locate(&Env::current()).map_err(|e| ("500 Internal Server Error", format!("找不到数据根：{e}")))?;
    let session = q.get("session").map(String::as_str).unwrap_or("");
    match route {
        "/file" => {
            let path = Path::new(q.get("path").map(String::as_str).unwrap_or(""));
            if !path.is_absolute() {
                return Err(("400 Bad Request", "要绝对路径".to_string()));
            }
            let real = path.canonicalize().map_err(|e| ("404 Not Found", format!("找不到：{e}")))?;
            let data = root.path().canonicalize().unwrap_or_else(|_| root.path().to_path_buf());
            if real.starts_with(&data) {
                return Err(("403 Forbidden", "这是 Miyu 自己的数据，不给".to_string()));
            }
            if !real.is_file() {
                return Err(("404 Not Found", "不是普通文件".to_string()));
            }
            let kind = site.types.of(&real);
            Ok((real, kind))
        }
        "/blob" => {
            let hash = q.get("hash").map(String::as_str).unwrap_or("");
            let hex = hash.strip_prefix("sha256:").filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
                .ok_or(("400 Bad Request", "哈希写得不对".to_string()))?;
            let events = history::read(&root, &account, session, 0).map_err(|e| ("404 Not Found", e))?;
            if !events.iter().any(|e| e.to_string().contains(hash)) {
                return Err(("404 Not Found", "这个会话里没有这个哈希".to_string()));
            }
            let kind = q.get("type").filter(|t| site.types.blob.contains(t)).cloned().unwrap_or_else(|| "application/octet-stream".to_string());
            let account = miyu_kernel::id::AccountId::parse(&account).map_err(|e| ("400 Bad Request", e.to_string()))?;
            Ok((root.blobs(&account).join(&hex[..2]).join(hex), kind))
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

/// 回文件：有 Range 的回那一段（206），没有的回全部；一块块照抄，不整个读进内存（视频有几个 G）。
async fn send(stream: &mut TcpStream, path: &Path, kind: &str, range: Option<(Option<u64>, Option<u64>)>, save: Option<&str>) -> io::Result<()> {
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return plain(stream, "404 Not Found", "打不开").await;
    };
    let total = file.metadata().await?.len();
    let (status, start, len) = match range {
        None => ("200 OK", 0, total),
        Some((a, b)) => {
            let (start, end) = match (a, b) {
                (Some(a), Some(b)) => (a, b.min(total.saturating_sub(1))),
                (Some(a), None) => (a, total.saturating_sub(1)),
                (None, Some(n)) => (total.saturating_sub(n), total.saturating_sub(1)),
                (None, None) => (0, total.saturating_sub(1)),
            };
            if total == 0 || start > end || start >= total {
                let head = format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                return stream.write_all(head.as_bytes()).await;
            }
            ("206 Partial Content", start, end - start + 1)
        }
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
    file.seek(io::SeekFrom::Start(start)).await?;
    tokio::io::copy(&mut file.take(len), stream).await?;
    Ok(())
}

/// 回一句纯文字（出错的时候）。`/link-image` 也用它。
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
