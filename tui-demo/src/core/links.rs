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

/// 核心交回的一张卡片。记进界面的缓存（`link_cards.rs`），重启以后照它画。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Card {
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
