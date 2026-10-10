//! 软件后台页（蓝图 `package-pages.md`「网页软件怎么给后台页」、`web-ui.md`；核心 F-6 中）：带自己页面的软件，网页把它的页面放在
//! 隔离的框里显示。页面的文件经核心拿（`package.file`），桥不读核心的存储。
//!
//! - `POST /page`：`Authorization: Bearer <访问口令>`，正文 `{"package"}`。照 `package.list` 查它有没有后台页，没有的 404；有的造一张
//!   票据，回 `{"url": "/p/<票据>/<包>/"}`。框的地址里只有票据、不带口令：框里的页面读得到自己的地址，口令不能落到它手里。
//! - `GET /p/<票据>/<包>/<路径>`：票据不认识、包对不上的 404；照 `package.file` 一块块读、一块块写。空路径、以 `/` 结尾的是
//!   `index.html`（核心认空路径）。类型照 `web.json` 的表，表里没有的 `application/octet-stream`。
//! - 响应头：沙箱、不许联网、只许嵌在网页里（`CSP`），加上 `nosniff`、`no-referrer`、不缓存；`Access-Control-Allow-Origin: *`：
//!   沙箱里的框来源是空的，ES 模块脚本、字体是按跨源取的，不给这一格取不到；票据就是凭据，不多开口子。
//! - 票据这一次启动有效，最多 `MOST` 张，多了扔掉最早的。

use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::Path;
use std::sync::Mutex;

use base64::Engine as _;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::Site;
use crate::core::{Core, Refused};
use crate::files::decode;
use crate::media::plain;

/// 最多留几张票据：多了扔掉最早造的。
const MOST: usize = 64;

/// 后台页的响应头里的内容安全策略：沙箱（能跑脚本、能交表单，来源是空的）、只取它自己的文件、不许联网、只许嵌在网页里。
const CSP: &str = "sandbox allow-scripts allow-forms; default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; \
    img-src 'self' data: blob:; font-src 'self' data:; connect-src 'none'; frame-ancestors 'self'";

/// 票据：哪张对哪个包，照造的先后。
#[derive(Default)]
pub struct Tickets {
    inner: Mutex<(HashMap<String, String>, VecDeque<String>)>,
}

impl Tickets {
    /// 给一个包造一张票据。
    fn issue(&self, package: &str) -> Option<String> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).ok()?;
        let ticket: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let mut inner = self.inner.lock().ok()?;
        let (map, order) = &mut *inner;
        map.insert(ticket.clone(), package.to_string());
        order.push_back(ticket.clone());
        while order.len() > MOST {
            if let Some(old) = order.pop_front() {
                map.remove(&old);
            }
        }
        Some(ticket)
    }

    /// 这张票据是哪个包的。
    fn package(&self, ticket: &str) -> Option<String> {
        self.inner.lock().ok()?.0.get(ticket).cloned()
    }
}

/// 页面文件的媒体类型（`resources/web/web.json` 的 `types`）。
pub struct PageTypes(HashMap<String, String>);

impl PageTypes {
    /// 读资源目录下的 `web/web.json`：资源目录照 `MIYU_RESOURCES`，没设的是仓库的 `resources/`。
    ///
    /// # Errors
    ///
    /// 读不到、不是 JSON、没有 `types`：说是哪一样。
    pub fn load() -> Result<PageTypes, String> {
        let root = std::env::var_os("MIYU_RESOURCES").map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"), Into::into);
        let file = root.join("web/web.json");
        let text = std::fs::read_to_string(&file).map_err(|e| format!("读不了 {}：{e}", file.display()))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{} 不是 JSON：{e}", file.display()))?;
        let types = v["types"].as_object().ok_or_else(|| format!("{} 少了 types", file.display()))?
            .iter().filter_map(|(k, t)| Some((k.clone(), t.as_str()?.to_string()))).collect();
        Ok(PageTypes(types))
    }

    fn of(&self, path: &str) -> &str {
        let ext = Path::new(path).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
        self.0.get(&ext).map_or("application/octet-stream", String::as_str)
    }
}

/// `POST /page`：口令对得上、这个包有后台页的，造一张票据。
///
/// # Errors
///
/// 写连接出错。
pub async fn issue(stream: &mut TcpStream, site: &Site, head: &str, body: &[u8]) -> io::Result<()> {
    let bearer = head.lines().find_map(|l| {
        let (name, value) = l.split_once(':')?;
        name.trim().eq_ignore_ascii_case("authorization").then(|| value.trim().strip_prefix("Bearer ").unwrap_or("").to_string())
    });
    if bearer.as_deref() != Some(site.key.as_str()) {
        return plain(stream, "403 Forbidden", "口令不对").await;
    }
    let Some(package) = serde_json::from_slice::<Value>(body).ok().and_then(|v| v["package"].as_str().map(str::to_string)) else {
        return plain(stream, "400 Bad Request", "要 {\"package\"}").await;
    };
    let mut core = match Core::open().await {
        Ok(core) => core,
        Err(why) => return plain(stream, "502 Bad Gateway", &why).await,
    };
    let listed = match core.call("package.list", json!({})).await {
        Ok(got) => got,
        Err(refused) => return plain(stream, "502 Bad Gateway", &refused.message).await,
    };
    let has_page = listed["packages"].as_array().is_some_and(|all| all.iter().any(|p| p["package"] == json!(package) && p["page"] == json!(true)));
    if !has_page {
        return plain(stream, "404 Not Found", "这个软件没有后台页").await;
    }
    let Some(ticket) = site.pages.issue(&package) else {
        return plain(stream, "500 Internal Server Error", "造不了票据").await;
    };
    let body = json!({"url": format!("/p/{ticket}/{}/", encode(&package))}).to_string();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await
}

/// `GET /p/<票据>/<包>/<路径>`：照票据给这个包后台页里的一份文件。
///
/// # Errors
///
/// 写连接出错。
pub async fn serve(stream: &mut TcpStream, site: &Site, path: &str) -> io::Result<()> {
    let Some((ticket, package, file)) = split(path) else {
        return plain(stream, "404 Not Found", "not found").await;
    };
    if site.pages.package(&ticket).as_deref() != Some(package.as_str()) {
        return plain(stream, "404 Not Found", "not found").await;
    }
    let mut core = match Core::open().await {
        Ok(core) => core,
        Err(why) => return plain(stream, "502 Bad Gateway", &why).await,
    };
    // 先读第一块：知道一共多大、有没有这份文件，再写响应头
    let (first, size, mut eof) = match read(&mut core, &package, &file, 0).await {
        Ok(got) => got,
        Err(refused) => return plain(stream, status_of(&refused.reason), &refused.message).await,
    };
    let kind = site.page_types.of(if file.is_empty() { "index.html" } else { &file });
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {size}\r\nContent-Security-Policy: {CSP}\r\nX-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&first).await?;
    let mut at = first.len() as u64;
    while !eof && at < size {
        // 头已经写出去了：中途出错只能断开，浏览器照长度对不上认出来
        let Ok((data, _, end)) = read(&mut core, &package, &file, at).await else { return Ok(()) };
        if data.is_empty() {
            return Ok(());
        }
        stream.write_all(&data).await?;
        at += data.len() as u64;
        eof = end;
    }
    Ok(())
}

/// 问核心要一块：这一块、整份多大、读到头了没有。
async fn read(core: &mut Core, package: &str, path: &str, offset: u64) -> Result<(Vec<u8>, u64, bool), Refused> {
    let got = core.call("package.file", json!({"package": package, "path": path, "offset": offset})).await?;
    let data = base64::engine::general_purpose::STANDARD
        .decode(got["data"].as_str().unwrap_or(""))
        .map_err(|e| Refused { reason: "bridge".to_string(), message: format!("核心给的不是 base64：{e}") })?;
    Ok((data, got["size"].as_u64().unwrap_or(0), got["eof"].as_bool().unwrap_or(true)))
}

/// `/p/<票据>/<包>/<路径>` 拆成三段（路径的 `%xx` 换回来；以 `/` 结尾的补上 `index.html`，空的照核心认成 `index.html`）。
fn split(path: &str) -> Option<(String, String, String)> {
    let rest = path.strip_prefix("/p/")?;
    let (ticket, rest) = rest.split_once('/')?;
    let (package, file) = rest.split_once('/').unwrap_or((rest, ""));
    if ticket.is_empty() || package.is_empty() {
        return None;
    }
    let mut file = decode(file);
    if file.ends_with('/') {
        file.push_str("index.html");
    }
    Some((ticket.to_string(), decode(package), file))
}

/// 包的编号放进地址：只有字母、数字、`-`、`_`、`.` 原样，别的转义。
fn encode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// 核心拒的原因码换成 HTTP 的状态。
fn status_of(reason: &str) -> &'static str {
    match reason {
        "bad_params" => "400 Bad Request",
        "not_found" | "no_page" | "unknown_package" => "404 Not Found",
        _ => "502 Bad Gateway",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_split_into_ticket_package_and_file() {
        assert_eq!(split("/p/abc/demo/"), Some(("abc".into(), "demo".into(), String::new())));
        assert_eq!(split("/p/abc/demo"), Some(("abc".into(), "demo".into(), String::new())));
        assert_eq!(split("/p/abc/demo/app.js"), Some(("abc".into(), "demo".into(), "app.js".into())));
        assert_eq!(split("/p/abc/demo/sub/"), Some(("abc".into(), "demo".into(), "sub/index.html".into())));
        assert_eq!(split("/p/abc/demo/%E4%B8%AD.css"), Some(("abc".into(), "demo".into(), "中.css".into())));
        assert_eq!(split("/p//demo/"), None);
        assert_eq!(split("/x/abc/demo/"), None);
    }

    #[test]
    fn tickets_name_their_package_and_old_ones_go() {
        let t = Tickets::default();
        let first = t.issue("a").unwrap_or_default();
        assert_eq!(t.package(&first).as_deref(), Some("a"));
        for _ in 0..MOST {
            let _ = t.issue("b");
        }
        assert_eq!(t.package(&first), None, "多了扔掉最早的");
        assert_eq!(t.package("nope"), None);
    }

    #[test]
    fn refusals_become_statuses() {
        assert_eq!(status_of("not_found"), "404 Not Found");
        assert_eq!(status_of("no_page"), "404 Not Found");
        assert_eq!(status_of("bad_params"), "400 Bad Request");
        assert_eq!(status_of("bridge"), "502 Bad Gateway");
    }
}
