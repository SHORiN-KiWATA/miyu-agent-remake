//! 链接卡片（蓝图 `tui.md`「链接卡片」，核心 W-7、W-6）：向核心要一个网址的卡片（`link.preview`，回应可能晚到，照请求
//! 编号认），卡片里的封面图、网站图标是这个账号的 blob，照 `blob.get` 一段段读回来（一次最多 512 KiB），存进临时
//! 目录里的一份文件，交给画图的那一层照本机的图片画。

use std::collections::HashMap;
use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use super::Update;
use super::awaiting::Awaiting;
use super::rpc::Rpc;

/// 一次读多少字节（核心的上限，`blob.get` 的 `length`）。
const CHUNK: usize = 512 * 1024;

/// 链接卡片的内容种类；旧缓存或未知种类照普通页面显示。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    /// 视频：B 站、已读到时长的 YouTube 页面。
    Video,
    /// 文章：MediaWiki 站点。
    Article,
    /// 普通页面，也是旧卡片的默认值。
    #[default]
    #[serde(other)]
    Page,
}

/// 核心交回的一张卡片。记进界面的缓存（`link_cards.rs`），重启以后照它画。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Card {
    /// 内容种类；旧缓存没有这一格时照页面。
    #[serde(default)]
    pub kind: CardKind,
    /// 视频时长，单位秒；没有或不是正整数的为无。
    #[serde(default)]
    pub duration: Option<u64>,
    /// 作者、UP 主或频道名；没有的为无。
    #[serde(default)]
    pub author: Option<String>,
    /// 标题。
    pub title: String,
    /// 简介。
    pub description: String,
    /// 网站名。
    pub site: String,
    /// 封面图的 blob。
    pub image: Option<String>,
    /// 网站图标的 blob。
    pub icon: Option<String>,
}

/// `link.preview` 的回应里的卡片；`card` 是 `null`（要不到）的是 `None`。
pub fn card(result: &Value) -> Option<Card> {
    let card = result.get("card").filter(|c| c.is_object())?;
    let text = |k: &str| card[k].as_str().unwrap_or_default().trim().to_string();
    let blob = |k: &str| card[k]["blob"].as_str().map(str::to_string);
    let out = Card {
        kind: match card["kind"].as_str() {
            Some("video") => CardKind::Video,
            Some("article") => CardKind::Article,
            _ => CardKind::Page,
        },
        duration: card["duration"].as_u64().filter(|seconds| *seconds > 0),
        author: card["author"]
            .as_str()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string),
        title: text("title"),
        description: text("description"),
        site: text("site"),
        image: blob("image"),
        icon: blob("icon"),
    };
    (!out.title.is_empty() || !out.site.is_empty()).then_some(out)
}

/// 读一个 blob 的第一段。
pub(super) async fn fetch(
    rpc: &mut Rpc,
    blob: &str,
    awaiting: &mut HashMap<String, Awaiting>,
) -> std::io::Result<()> {
    let id = rpc
        .send(
            "blob.get",
            json!({"blob": blob, "offset": 0, "length": CHUNK}),
        )
        .await?;
    awaiting.insert(id, Awaiting::Blob(blob.to_string(), Vec::new()));
    Ok(())
}

/// 读回一段：接上，没读完的接着要下一段，读完了存成文件交给界面（读不成、存不成的交回 `None`）。交回界面还在不在。
pub(super) async fn chunk(
    rpc: &mut Rpc,
    blob: String,
    mut got: Vec<u8>,
    result: &Value,
    awaiting: &mut HashMap<String, Awaiting>,
    notify: &impl Fn(Update) -> bool,
) -> bool {
    let data = result["data"]
        .as_str()
        .and_then(|d| STANDARD.decode(d).ok())
        .unwrap_or_default();
    let size = result["size"].as_u64().unwrap_or(0);
    got.extend_from_slice(&data);
    if !data.is_empty() && (got.len() as u64) < size {
        let offset = got.len();
        let sent = rpc
            .send(
                "blob.get",
                json!({"blob": blob, "offset": offset, "length": CHUNK}),
            )
            .await;
        if let Ok(id) = sent {
            awaiting.insert(id, Awaiting::Blob(blob, got));
            return true;
        }
    }
    let path = (!got.is_empty()).then(|| save(&blob, &got)).flatten();
    notify(Update::BlobSaved { blob, path })
}

/// 存进缓存目录里的一份文件（[`blob_path`]）；存不成的是 `None`。
fn save(blob: &str, data: &[u8]) -> Option<PathBuf> {
    let path = blob_path(blob)?;
    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::write(&path, data).ok()?;
    Some(path)
}

/// 一个 blob 存在哪：机器共用的缓存目录下 `tui/link-cards/blobs/`，名字照 blob（内容的哈希，只留字母数字）。同一份
/// 内容只存一次，重启以后照样在（2026-10-02 项目主人报：重启以后链接都要重新出卡片）。
pub fn blob_path(blob: &str) -> Option<PathBuf> {
    let root = miyu_store::root::cache_root(&miyu_store::env::Env::current()).ok()?;
    let name: String = blob.chars().filter(char::is_ascii_alphanumeric).collect();
    Some(cards_dir(&root).join("blobs").join(name))
}

/// 链接卡片的缓存目录。
pub fn cards_dir(cache_root: &std::path::Path) -> PathBuf {
    cache_root.join("tui").join("link-cards")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::card;

    #[test]
    fn a_card_reads_its_text_and_blobs_and_null_is_none() {
        let got = card(&json!({"card": {"title": " 标题 ", "description": "简介", "site": "example.com",
            "url": "https://example.com", "image": {"blob": "sha256:aa", "media_type": "image/png"}, "icon": null}}))
        .unwrap();
        assert_eq!(got.title, "标题");
        assert_eq!(got.image.as_deref(), Some("sha256:aa"));
        assert_eq!(got.icon, None);
        assert_eq!(card(&json!({"card": null, "why": "no_preview"})), None);
    }
}

#[cfg(test)]
mod site_card_tests {
    use super::{Card, card};
    use serde_json::json;

    #[test]
    fn site_card_reads_video_metadata_and_rejects_invalid_duration() {
        let base = json!({"card":{"title":"视频", "site":"哔哩哔哩", "kind":"video", "duration":408, "author":" 明日方舟 "}});
        let got = serde_json::to_value(card(&base).unwrap()).unwrap();
        assert_eq!(got["kind"], "video");
        assert_eq!(got["duration"], 408);
        assert_eq!(got["author"], "明日方舟");
        for duration in [
            json!(0),
            json!(-1),
            json!(2.5),
            json!("408"),
            json!(null),
            json!(true),
        ] {
            let mut bad = base.clone();
            bad["card"]["duration"] = duration;
            let got = serde_json::to_value(card(&bad).unwrap()).unwrap();
            assert!(got["duration"].is_null());
        }
    }

    #[test]
    fn old_card_fields_still_deserialize_and_missing_metadata_defaults() {
        let old =
            json!({"title":"旧卡片", "description":"", "site":"旧网站", "image":null, "icon":null});
        let got: Card = serde_json::from_value(old).unwrap();
        let got = serde_json::to_value(got).unwrap();
        assert_eq!(got["kind"], "page");
        assert!(got["duration"].is_null());
        assert!(got["author"].is_null());
        let got =
            serde_json::to_value(card(&json!({"card":{"title":"标题", "author":"  "}})).unwrap())
                .unwrap();
        assert_eq!(got["kind"], "page");
        assert!(got["author"].is_null());
    }
}
