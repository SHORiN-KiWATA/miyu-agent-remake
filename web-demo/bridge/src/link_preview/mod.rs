//! 链接卡片（旧版 `miyu-hosts` 的 `web/link_preview`，规矩照搬）：`web.link_preview` 回一页的元数据，`/link-image` 给它的图。
//!
//! 正文里单独占一行的链接，页面拿这里的元数据升级成一张带图的卡片；失败、超时、页面没有能用的元数据时，页面原样留着那条
//! 链接：卡片是锦上添花，哪一环出问题都不该让正文变样。所以 `web.link_preview` 总是成功的回应，`{ok: true, preview}` 或
//! `{ok: false, reason}`（旧版的接口永远回 200）：做不出卡片是正常结果之一，不是错误。
//!
//! 顶替核心以后的查询 `link.preview`（`net` 包，和 web_fetch 共用抓取和 SSRF 闸；2026-09-30 核心施工定），那边做好以后
//! 删掉这一份。分四个文件：这里是入口和两份缓存，`fetch.rs` 出站抓取，`html.rs` 挖元数据，`guard.rs` 地址闸；数都在
//! `resources/link_preview.json`。
//!
//! - 抓过的记着：抓到了的记得久（页面的 og 标签基本不动）；做不出卡片的（不是网页、没有标题、地址不合规）记一会儿，这种
//!   结论不会自己变；网络抖了的（超时、连不上、对面临时 5xx）只记很短：会自己变好，记久了等于把一条本来能出卡片的链接
//!   按死（旧版 09-09 那条 bilibili 就是这么变成纯文字的）。满了整个清空：几百字节一条的表，清空的代价只是重抓一次。
//! - 图只记在内存里（旧版落在缓存目录），编号是内容的 SHA-256：同一张图只记一份，编号不带来源。数量、总字节有上限，满了
//!   先进先出；记着的卡片指着的图被挤掉了的，回卡片时那一格给 `null`，页面画没有图的小卡。

mod fetch;
mod guard;
mod html;

use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::Url;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::{Site, files::decode, media};

/// 做不出卡片的两种，记的时长不同。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Miss {
    /// 不会自己变好：页面没有能用的元数据、不是网页、地址过不了闸、重定向太多。
    NoPreview,
    /// 网络抖了：超时、连不上、对面回 4xx/5xx，下次可能就好了。
    Transient,
}

/// 一张卡片：图和图标是 `/link-image` 的编号。
#[derive(Clone, Debug, PartialEq, Eq)]
struct Card {
    url: String,
    title: String,
    description: String,
    site: String,
    image: Option<String>,
    icon: Option<String>,
}

/// 记着的一条：结果和什么时候记的。
struct Remembered {
    value: Result<Card, Miss>,
    at: Instant,
}

/// 记着的一张图：照魔数认出来的媒体类型和字节。
#[derive(Clone)]
struct Image {
    kind: &'static str,
    bytes: Arc<[u8]>,
}

/// 内存里的图：编号 → 图，先进先出。
#[derive(Default)]
struct Images {
    order: VecDeque<String>,
    by_id: HashMap<String, Image>,
    bytes: usize,
}

/// 链接卡片：规矩（`resources/link_preview.json`）、抓过的、抓回来的图。
pub struct LinkPreview {
    fetcher: fetch::Fetcher,
    clip: html::Clip,
    found_ttl: Duration,
    no_preview_ttl: Duration,
    transient_ttl: Duration,
    entries: usize,
    image_keep: usize,
    image_keep_bytes: usize,
    cache_control: String,
    remembered: Mutex<HashMap<String, Remembered>>,
    images: Mutex<Images>,
}

impl LinkPreview {
    /// 读页面目录下的 `resources/link_preview.json`。
    ///
    /// # Errors
    ///
    /// 读不到、不是 JSON、少了哪一格：说是哪一样。
    pub fn load(dir: &Path) -> Result<LinkPreview, String> {
        let file = dir.join("resources/link_preview.json");
        let text = std::fs::read_to_string(&file).map_err(|e| format!("读不了 {}：{e}", file.display()))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{} 不是 JSON：{e}", file.display()))?;
        let bad = |k: &str| format!("{} 少了 {k}", file.display());
        let number = |k: &str| v.pointer(k).and_then(Value::as_u64).ok_or_else(|| bad(k));
        let n = |k: &str| number(k).and_then(|n| usize::try_from(n).map_err(|_| bad(k)));
        let seconds = |k: &str| number(k).map(Duration::from_secs);
        let s = |k: &str| v.pointer(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| bad(k));
        let rules = fetch::Rules {
            page_timeout: seconds("/page/timeout_seconds")?,
            page_bytes: n("/page/max_bytes")?,
            page_accept: s("/page/accept")?,
            image_timeout: seconds("/image/timeout_seconds")?,
            image_bytes: n("/image/max_bytes")?,
            image_accept: s("/image/accept")?,
            redirects: n("/redirects")?,
            user_agent: s("/user_agent")?,
            accept_language: s("/accept_language")?,
            client_ttl: seconds("/clients/seconds")?,
            client_keep: n("/clients/keep")?,
        };
        Ok(LinkPreview {
            fetcher: fetch::Fetcher::new(rules),
            clip: html::Clip { title: n("/clip/title")?, description: n("/clip/description")?, site: n("/clip/site")? },
            found_ttl: seconds("/remember/found_seconds")?,
            no_preview_ttl: seconds("/remember/no_preview_seconds")?,
            transient_ttl: seconds("/remember/transient_seconds")?,
            entries: n("/remember/entries")?,
            image_keep: n("/image/keep")?,
            image_keep_bytes: n("/image/keep_bytes")?,
            cache_control: format!("private, max-age={}, immutable", number("/image/browser_cache_seconds")?),
            remembered: Mutex::new(HashMap::new()),
            images: Mutex::new(Images::default()),
        })
    }

    /// `web.link_preview`：`{ok: true, preview: {url, title, description, site, image, icon}}`（图和图标是 `/link-image`
    /// 的编号或 `null`），做不出卡片的 `{ok: false, reason}`。抓过的照记着的给。
    pub async fn preview(&self, url: &str) -> Value {
        let Ok(url) = Url::parse(url.trim()) else {
            return refused("not a URL");
        };
        if !matches!(url.scheme(), "http" | "https") {
            return refused("unsupported scheme");
        }
        let key = url.as_str().to_string();
        if let Some(hit) = self.cached(&key) {
            return self.answer(&hit);
        }
        let outcome = self.fetch(&url).await;
        self.remember(key, outcome.clone());
        self.answer(&outcome)
    }

    /// 抓一页做成卡片：一张卡至少要有个标题，不然不如留着原来的链接。
    async fn fetch(&self, url: &Url) -> Result<Card, Miss> {
        let (page, head) = self.fetcher.page(url).await?;
        let found = html::read(&head, &page, &self.clip);
        if found.title.is_empty() {
            return Err(Miss::NoPreview);
        }
        // 两张图互不相干，一起抓
        let (image, icon) = tokio::join!(self.keep_image(found.image.as_ref()), self.keep_image(found.icon.as_ref()));
        Ok(Card { url: page.to_string(), title: found.title, description: found.description, site: found.site, image, icon })
    }

    /// 抓一张图记下来，交回编号；抓不到的没有（卡片照样成立）。
    async fn keep_image(&self, url: Option<&Url>) -> Option<String> {
        let (bytes, kind) = self.fetcher.image(url?).await?;
        self.store(bytes, kind)
    }

    /// 记一张图，交回它的编号（内容的 SHA-256，十六进制）；记过的不重记。放不下就先挤掉最早的。
    fn store(&self, bytes: Vec<u8>, kind: &'static str) -> Option<String> {
        if self.image_keep == 0 || bytes.len() > self.image_keep_bytes {
            return None;
        }
        let id: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        let mut images = self.images.lock().ok()?;
        if images.by_id.contains_key(&id) {
            return Some(id);
        }
        while images.by_id.len() >= self.image_keep || images.bytes + bytes.len() > self.image_keep_bytes {
            let Some(oldest) = images.order.pop_front() else { break };
            if let Some(gone) = images.by_id.remove(&oldest) {
                images.bytes -= gone.bytes.len();
            }
        }
        images.bytes += bytes.len();
        images.order.push_back(id.clone());
        images.by_id.insert(id.clone(), Image { kind, bytes: Arc::from(bytes) });
        Some(id)
    }

    /// 记着的、还没过期的一条。
    fn cached(&self, key: &str) -> Option<Result<Card, Miss>> {
        let remembered = self.remembered.lock().ok()?;
        let entry = remembered.get(key)?;
        let ttl = match entry.value {
            Ok(_) => self.found_ttl,
            Err(Miss::NoPreview) => self.no_preview_ttl,
            Err(Miss::Transient) => self.transient_ttl,
        };
        (entry.at.elapsed() < ttl).then(|| entry.value.clone())
    }

    /// 记一条；满了整个清空（不值得做 LRU：清空的代价只是重抓一次）。
    fn remember(&self, key: String, value: Result<Card, Miss>) {
        let Ok(mut remembered) = self.remembered.lock() else { return };
        if remembered.len() >= self.entries {
            remembered.clear();
        }
        remembered.insert(key, Remembered { value, at: Instant::now() });
    }

    /// 结果写成回给页面的样子：图已经被挤掉的那一格给 `null`。
    fn answer(&self, value: &Result<Card, Miss>) -> Value {
        let Ok(card) = value else {
            return refused("no preview available");
        };
        let images = self.images.lock().ok();
        let still = |id: &Option<String>| id.clone().filter(|id| images.as_ref().is_some_and(|i| i.by_id.contains_key(id)));
        json!({"ok": true, "preview": {
            "url": card.url, "title": card.title, "description": card.description, "site": card.site,
            "image": still(&card.image), "icon": still(&card.icon),
        }})
    }

    /// `/link-image` 的查询串对上口令、找到这张图才给；不给的交回状态和为什么。
    fn image(&self, key: &str, query: &str) -> Result<Image, (&'static str, &'static str)> {
        let q: HashMap<String, String> = query
            .split('&')
            .filter_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                Some((decode(k), decode(&v.replace('+', " "))))
            })
            .collect();
        if q.get("k").map(String::as_str) != Some(key) {
            return Err(("403 Forbidden", "口令不对"));
        }
        let id = q.get("id").map(String::as_str).unwrap_or_default();
        let found = self.images.lock().ok().and_then(|images| images.by_id.get(id).cloned());
        found.ok_or(("404 Not Found", "没有这张图"))
    }
}

/// 做不出卡片的回应。
fn refused(reason: &str) -> Value {
    json!({"ok": false, "reason": reason})
}

/// `GET /link-image?k=<口令>&id=<编号>`：给一张记着的图，带 `nosniff`；编号是内容的哈希，浏览器可以放心长缓存。
/// 口令不对 403，没有这张 404。
///
/// # Errors
///
/// 写连接出错。
pub async fn serve(stream: &mut TcpStream, site: &Site, query: &str) -> io::Result<()> {
    let image = match site.link_preview.image(&site.key, query) {
        Ok(image) => image,
        Err((status, why)) => return media::plain(stream, status, why).await,
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: {}\r\nConnection: close\r\n\r\n",
        image.kind,
        image.bytes.len(),
        site.link_preview.cache_control
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&image.bytes).await?;
    stream.shutdown().await
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

    fn links() -> LinkPreview {
        LinkPreview::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).expect("resources/link_preview.json 读得懂")
    }

    fn card(image: Option<String>) -> Card {
        let text = String::from("x");
        Card { url: text.clone(), title: text.clone(), description: text.clone(), site: text, image, icon: None }
    }

    #[test]
    fn a_hiccup_is_not_pinned_as_long_as_a_real_answer() {
        let links = links();
        assert!(links.transient_ttl < links.no_preview_ttl);
        assert!(links.no_preview_ttl < links.found_ttl);
    }

    #[test]
    fn a_full_cache_is_cleared_rather_than_grown() {
        let links = links();
        for index in 0..=links.entries {
            links.remember(format!("https://example.com/{index}"), Err(Miss::NoPreview));
        }
        assert!(links.remembered.lock().map(|r| r.len()).unwrap_or(usize::MAX) <= links.entries);
        assert_eq!(links.cached(&format!("https://example.com/{}", links.entries)), Some(Err(Miss::NoPreview)));
    }

    #[test]
    fn images_are_bounded_by_count_and_bytes_oldest_first() {
        let image = |n: u8| [PNG, &[n]].concat();
        let has = |links: &LinkPreview, id: &Option<String>| {
            id.as_ref().is_some_and(|id| links.images.lock().is_ok_and(|i| i.by_id.contains_key(id)))
        };
        // 按张数：满了挤掉最早的；同一张图只记一份
        let mut links = links();
        links.image_keep = 2;
        let first = links.store(image(1), "image/png");
        assert_eq!(links.store(image(1), "image/png"), first);
        let second = links.store(image(2), "image/png");
        let third = links.store(image(3), "image/png");
        assert!(!has(&links, &first) && has(&links, &second) && has(&links, &third));
        // 按总字节：放不下就挤掉最早的；比总上限还大的不记
        let mut links = self::links();
        links.image_keep_bytes = image(0).len() * 2;
        let first = links.store(image(1), "image/png");
        let second = links.store(image(2), "image/png");
        let third = links.store(image(3), "image/png");
        assert!(!has(&links, &first) && has(&links, &second) && has(&links, &third));
        assert_eq!(links.store(vec![0; links.image_keep_bytes + 1], "image/png"), None);
        assert!(links.images.lock().is_ok_and(|i| i.bytes == image(0).len() * 2 && i.order.len() == i.by_id.len()));
    }

    #[test]
    fn link_image_checks_the_key_then_the_id() {
        let links = links();
        let id = links.store(PNG.to_vec(), "image/png").unwrap_or_default();
        let got = links.image("secret", &format!("k=secret&id={id}"));
        assert!(got.as_ref().is_ok_and(|i| i.kind == "image/png" && &*i.bytes == PNG));
        for (query, status) in [
            (format!("id={id}"), "403 Forbidden"),
            (format!("k=wrong&id={id}"), "403 Forbidden"),
            (format!("k=secre&id={id}"), "403 Forbidden"),
            ("k=secret".to_string(), "404 Not Found"),
            (format!("k=secret&id={}", "0".repeat(64)), "404 Not Found"),
            ("k=secret&id=..%2F..%2Fetc%2Fpasswd".to_string(), "404 Not Found"),
        ] {
            assert_eq!(links.image("secret", &query).err().map(|(s, _)| s), Some(status), "{query}");
        }
    }

    #[test]
    fn a_card_whose_image_was_pushed_out_says_null() {
        let links = links();
        let kept = links.store(PNG.to_vec(), "image/png");
        let shown = links.answer(&Ok(card(kept.clone())));
        assert_eq!(shown["ok"], true);
        assert_eq!(shown["preview"]["image"], json!(kept));
        let gone = links.answer(&Ok(card(Some("f".repeat(64)))));
        assert_eq!(gone["preview"]["image"], Value::Null);
        assert_eq!(gone["preview"]["icon"], Value::Null);
        assert_eq!(links.answer(&Err(Miss::Transient)), refused("no preview available"));
    }

    #[tokio::test]
    async fn bad_addresses_are_refused_without_touching_the_network() {
        let links = links();
        assert_eq!(links.preview("not a url").await, refused("not a URL"));
        assert_eq!(links.preview("javascript:alert(1)").await, refused("unsupported scheme"));
        assert_eq!(links.preview("ftp://example.com/").await, refused("unsupported scheme"));
        // 本机、内网的过不了闸：不查 DNS、不连，当「做不出卡片」记着
        for url in ["  http://127.0.0.1:8765/#k=secret  ", "http://localhost/", "http://[::1]/", "http://192.168.1.1/admin"] {
            assert_eq!(links.preview(url).await, refused("no preview available"), "{url}");
            let key = Url::parse(url.trim()).map(|u| u.to_string()).unwrap_or_default();
            assert_eq!(links.cached(&key), Some(Err(Miss::NoPreview)), "{url}");
        }
    }
}
