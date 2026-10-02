//! 链接卡片的账（蓝图 `tui.md`「链接卡片」第 2、4 条）：每个网址的卡片要过没有、要回来什么，每个 blob 存成了哪份文件。
//! 排正文时问一声，没要过的记进要发的单子；主循环每一帧以后把单子交给核心（`app/cards.rs`）。
//!
//! 要到的卡片记进缓存目录里的 `cards.json`（最近的 [`KEEP`] 个网址），图照内容的哈希存成文件（`core/links.rs`）：重启
//! 以后照记着的直接画，不先显示成链接再换（2026-10-02 项目主人报）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::Card;

/// 缓存里最多记几个网址的卡片。
const KEEP: usize = 500;

/// 一个网址的卡片现在怎样。
#[derive(Debug, Clone)]
enum Asked {
    /// 要了，还没回来。
    Waiting,
    /// 回来了；要不到的是 `None`（照旧显示成链接）。
    Got(Option<Card>),
}

/// 账。
#[derive(Debug, Default)]
pub struct LinkCards {
    cards: HashMap<String, Asked>,
    /// 读回来存成的文件；要了还没回来、读不成的是 `None`。
    blobs: HashMap<String, Option<PathBuf>>,
    /// 还没发的：要卡片的网址。
    ask_cards: Vec<String>,
    /// 还没发的：要读的 blob。
    ask_blobs: Vec<String>,
    /// 缓存文件（`cards.json`）；测试里没有。
    file: Option<PathBuf>,
    /// 记着的卡片的网址，照要到的先后（多了从前面丢）。
    order: Vec<String>,
}

impl LinkCards {
    /// 照机器共用的缓存目录读回以前要到的卡片；找不到缓存目录的当空的。
    pub fn cached() -> Self {
        miyu_store::root::cache_root(&miyu_store::env::Env::current())
            .map(|root| Self::open(crate::core::cards_dir(&root)))
            .unwrap_or_default()
    }

    /// 照缓存文件（`<缓存目录>/cards.json`）读回以前要到的卡片；读不了的当空的。
    pub fn open(dir: PathBuf) -> Self {
        let file = dir.join("cards.json");
        let saved: Vec<(String, Card)> = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let mut book = Self {
            file: Some(file),
            ..Self::default()
        };
        for (url, card) in saved {
            book.order.push(url.clone());
            book.cards.insert(url, Asked::Got(Some(card)));
        }
        book
    }

    /// 这个网址的卡片；没要过的记进单子，还没回来、要不到的是 `None`。
    pub fn card(&mut self, url: &str) -> Option<&Card> {
        if !self.cards.contains_key(url) {
            self.cards.insert(url.to_string(), Asked::Waiting);
            self.ask_cards.push(url.to_string());
        }
        match self.cards.get(url) {
            Some(Asked::Got(Some(card))) => Some(card),
            _ => None,
        }
    }

    /// 这个 blob 存成的文件；没要过的记进单子，还没回来、读不成的是 `None`。
    pub fn file(&mut self, blob: &str) -> Option<&Path> {
        if !self.blobs.contains_key(blob) {
            // 以前存过的（缓存目录里有）：直接用，不再要。
            let kept = self
                .file
                .as_ref()
                .and_then(|_| crate::core::blob_path(blob))
                .filter(|p| p.is_file());
            if kept.is_none() {
                self.ask_blobs.push(blob.to_string());
            }
            self.blobs.insert(blob.to_string(), kept);
        }
        self.blobs.get(blob).and_then(|p| p.as_deref())
    }

    /// 单子上有没有还没发的。
    pub fn pending(&self) -> bool {
        !self.ask_cards.is_empty() || !self.ask_blobs.is_empty()
    }

    /// 拿走单子：要卡片的网址、要读的 blob。
    pub fn take(&mut self) -> (Vec<String>, Vec<String>) {
        (
            std::mem::take(&mut self.ask_cards),
            std::mem::take(&mut self.ask_blobs),
        )
    }

    /// 核心交回了一张卡片（`None` 是要不到）。
    pub fn got_card(&mut self, url: String, card: Option<Card>) {
        let keep = card.is_some();
        self.cards.insert(url.clone(), Asked::Got(card));
        if keep {
            self.order.retain(|u| *u != url);
            self.order.push(url);
            self.save();
        }
    }

    /// 要到的卡片写回缓存文件（最近的 [`KEEP`] 个）；写不成的算了，下次启动再要。
    fn save(&mut self) {
        let Some(file) = &self.file else {
            return;
        };
        let extra = self.order.len().saturating_sub(KEEP);
        self.order.drain(..extra);
        let saved: Vec<(&String, &Card)> = self
            .order
            .iter()
            .filter_map(|u| match self.cards.get(u) {
                Some(Asked::Got(Some(card))) => Some((u, card)),
                _ => None,
            })
            .collect();
        let Ok(text) = serde_json::to_vec(&saved) else {
            return;
        };
        if let Some(dir) = file.parent()
            && std::fs::create_dir_all(dir).is_ok()
        {
            // 写不成的不管：下次启动再要。
            std::fs::write(file, text).unwrap_or_default();
        }
    }

    /// 一个 blob 存成了文件（`None` 是读不成）。
    pub fn saved(&mut self, blob: String, path: Option<PathBuf>) {
        self.blobs.insert(blob, path);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::LinkCards;
    use crate::core::Card;

    #[test]
    fn cards_are_kept_on_disk_and_read_back_after_a_restart() {
        // 2026-10-02 项目主人报：重启以后链接都要重新出卡片。
        let dir = std::env::temp_dir().join(format!("miyu-cards-{}", std::process::id()));
        let card = Card {
            title: "A".into(),
            description: String::new(),
            site: "a.dev".into(),
            image: None,
            icon: None,
        };
        let mut book = LinkCards::open(dir.clone());
        book.card("https://a.dev");
        book.got_card("https://a.dev".into(), Some(card));
        book.got_card("https://none.dev".into(), None);
        let mut again = LinkCards::open(dir.clone());
        assert_eq!(
            again.card("https://a.dev").map(|c| c.title.as_str()),
            Some("A")
        );
        assert!(again.take().0.is_empty(), "记着的不再要");
        assert!(
            again.card("https://none.dev").is_none(),
            "要不到的不记，下次再要"
        );
        std::fs::remove_dir_all(&dir).unwrap_or_default();
    }

    #[test]
    fn each_url_and_blob_is_asked_once_and_answers_are_kept() {
        let mut book = LinkCards::default();
        assert!(book.card("https://a.dev").is_none());
        assert!(book.card("https://a.dev").is_none(), "还没回来");
        assert!(book.file("sha256:aa").is_none());
        let (cards, blobs) = book.take();
        assert_eq!(cards, ["https://a.dev"], "同一个网址只要一次");
        assert_eq!(blobs, ["sha256:aa"]);
        assert!(!book.pending());
        let card = Card {
            title: "A".into(),
            description: String::new(),
            site: "a.dev".into(),
            image: None,
            icon: None,
        };
        book.got_card("https://a.dev".into(), Some(card));
        assert_eq!(
            book.card("https://a.dev").map(|c| c.title.as_str()),
            Some("A")
        );
        book.saved("sha256:aa".into(), Some(PathBuf::from("/tmp/x")));
        assert_eq!(book.file("sha256:aa"), Some(std::path::Path::new("/tmp/x")));
        assert!(!book.pending());
    }
}
