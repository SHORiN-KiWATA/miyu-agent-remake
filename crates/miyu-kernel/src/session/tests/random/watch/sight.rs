//! 看守替它看图（施工 8-17，`docs/blueprint/kernel/session.md`「替它看图」）：
//!
//! - 交出转述时：限额说看不了图，回合开着，这张图没转述过、没在路上，请求里是这张图；
//! - 记 `image.described` 时：内核记的，不带回合编号，转述就是送回去的那一句，同一张图只记一次；
//! - 请求模型时：`described` 正好是请求里转述过的图的转述；看不了图的主请求，里面的图都转述过，或者这一轮转述没成。

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::block::Image;
use crate::event::ImageDescribed;

/// 替它看图的账。
#[derive(Default)]
pub(in super::super) struct Sight {
    /// 交出去、还没送回的转述，和交出它的那一轮。
    pub(in super::super) looking: BTreeMap<ContentHash, TurnId>,
    /// 送回了成了的、内核还没记下的：blob → 转述。
    answered: BTreeMap<ContentHash, String>,
    /// 日志里转述过的：blob → 转述。
    described: BTreeMap<ContentHash, String>,
    /// 每一轮转述没成的。
    failed: BTreeMap<TurnId, BTreeSet<ContentHash>>,
}

impl Watch {
    /// 限额说看不了图。
    fn blind(&self) -> bool {
        self.compactions
            .limits
            .as_ref()
            .is_some_and(|limits| limits.blind)
    }

    /// 送进一个转述之前（`feed` 叫）：在路上的才算，成了的记下转述，没成的记在开着的那一轮上。
    pub(super) fn sight_fed(&mut self, input: &Input) {
        let Input::Described { blob, seen, .. } = input else {
            return;
        };
        let Some(asked) = self.sight.looking.remove(blob) else {
            self.seen_paths.insert("对不上的转述不理");
            return;
        };
        if self.undo.reading.is_some() {
            self.seen_paths.insert("读回日志时到的转述先放着");
        }
        match seen {
            Some((_, text)) => {
                if !self.turn_open() || self.open_turn() != asked {
                    self.seen_paths.insert("回合过去了才回来的转述");
                }
                self.sight.answered.insert(blob.clone(), text.clone());
            }
            None if self.turn_open() => {
                let turn = self.open_turn();
                self.sight
                    .failed
                    .entry(turn)
                    .or_default()
                    .insert(blob.clone());
            }
            None => {}
        }
    }

    /// 交出一次转述。
    pub(super) fn describe_issued(&mut self, blob: ContentHash, request: &Request) {
        let seed = self.seed;
        self.seen_paths.insert("交出了转述");
        assert!(self.blind(), "种子 {seed}：看得了图却转述");
        assert!(self.turn_open(), "种子 {seed}：没有回合却转述");
        assert!(
            !self.sight.described.contains_key(&blob),
            "种子 {seed}：{blob} 转述过了又转述"
        );
        let turn = self.open_turn();
        assert!(
            self.sight.looking.insert(blob.clone(), turn).is_none(),
            "种子 {seed}：{blob} 在路上又转述"
        );
        assert!(
            images(request).iter().any(|image| image.blob == blob),
            "种子 {seed}：转述的请求里没有这张图"
        );
    }

    /// 追加了一条 `image.described`。
    pub(super) fn described_appended(&mut self, event: &Event, described: &ImageDescribed) {
        let seed = self.seed;
        self.seen_paths.insert("记下了转述");
        assert_eq!(event.by, By::Kernel, "种子 {seed}：转述是内核记的");
        assert_eq!(event.turn, None, "种子 {seed}：转述不带回合编号");
        assert!(
            !self.sight.described.contains_key(&described.blob),
            "种子 {seed}：{} 记了两次转述",
            described.blob
        );
        assert_eq!(
            self.sight.answered.remove(&described.blob).as_deref(),
            Some(described.text.as_str()),
            "种子 {seed}：记下的不是送回去的那一句"
        );
        self.sight
            .described
            .insert(described.blob.clone(), described.text.clone());
    }

    /// 请求模型时：`described` 照日志里转述过的；看不了图的主请求，里面的图都转述过或者这一轮没成。
    pub(super) fn sight_called(&mut self, request: &Request, summary: bool) {
        let seed = self.seed;
        let pictures = images(request);
        let expected: BTreeMap<ContentHash, String> = pictures
            .iter()
            .filter_map(|image| {
                let text = self.sight.described.get(&image.blob)?;
                Some((image.blob.clone(), text.clone()))
            })
            .collect();
        assert_eq!(
            request.described, expected,
            "种子 {seed}：请求的转述不是日志里这几张图的"
        );
        if !expected.is_empty() {
            self.seen_paths.insert("请求里换上了转述");
        }
        if summary || !self.blind() || !self.turn_open() {
            return;
        }
        let failed = self.sight.failed.get(&self.open_turn());
        for image in &pictures {
            let written = expected.contains_key(&image.blob);
            let unseen = failed.is_some_and(|failed| failed.contains(&image.blob));
            assert!(
                written || unseen,
                "种子 {seed}：看不了图，{} 没转述就请求",
                image.blob
            );
            if unseen && !written {
                self.seen_paths.insert("转述没成写占位");
            }
        }
    }

    /// 崩了、重启以后：在路上的、送回了还没记下的跟着丢了。
    pub(super) fn sight_reloaded(&mut self) {
        self.sight.looking.clear();
        self.sight.answered.clear();
    }
}

/// 请求里 user、tool 消息的图。
fn images(request: &Request) -> Vec<Image> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } | Message::Tool { blocks, .. } => blocks.as_slice(),
            Message::Assistant { .. } => &[],
        })
        .filter_map(|block| match block {
            Block::Image(image) => Some(image.clone()),
            _ => None,
        })
        .collect()
}
