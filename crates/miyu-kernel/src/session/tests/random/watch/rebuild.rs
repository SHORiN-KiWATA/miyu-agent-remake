//! 看守查压后重建（`docs/blueprint/compaction.md` 第九条，施工 6-5）：
//!
//! - 交出的重读紧跟着同一个 `seen` 的摘要请求（`watch.rs` 的 `check`）；候选不空、不重复；
//! - 写下的压缩：没收到重读结果的，一个都不重读；收到了的，重读的是读到了的那几个，照先后，不超过策略里的个数、单个、
//!   合计，`tokens` 照原文估；代码写的几段带着取回指路。

use super::*;
use crate::event::{ContextCompacted, RestoredFile};
use crate::id::ContentHash;
use crate::session::Reread;

/// 看守记着的压后重建。
#[derive(Default)]
pub(in super::super) struct Rebuilds {
    /// 交出的重读：哪一次摘要请求、候选。
    asked: Option<(Seq, Vec<String>)>,
    /// 那次摘要请求还在路上时收到的重读结果。
    answered: Option<Vec<Reread>>,
    /// 送过一次结果了（随机的输入不再送）。
    fed: bool,
    /// 读到过的原文，照 blob：取回原文时交回去。
    pub(in super::super) texts: BTreeMap<ContentHash, String>,
    /// 内核要取回原文的那几份，还没交回（施工 6-9）：执行器做完才收收件箱，看守送完这一条输入马上交回。
    pub(super) recalling: Option<Vec<ContentHash>>,
}

impl Watch {
    /// 交出了一次重读：候选不空、不重复；紧跟着的下一个动作得是同一个 `seen` 的摘要请求。
    pub(super) fn reread_issued(&mut self, seen: Seq, paths: &[String]) {
        let seed = self.seed;
        assert!(!paths.is_empty(), "种子 {seed}：交出了空的重读");
        let distinct: BTreeSet<&String> = paths.iter().collect();
        assert_eq!(distinct.len(), paths.len(), "种子 {seed}：重读的候选重复了");
        self.compactions.reread = Some(seen);
        self.compactions.rebuild = Rebuilds {
            asked: Some((seen, paths.to_vec())),
            texts: std::mem::take(&mut self.compactions.rebuild.texts),
            recalling: self.compactions.rebuild.recalling.take(),
            ..Rebuilds::default()
        };
    }

    /// 还没送结果的那次重读：随机的输入照它回。
    pub(in super::super) fn reread_pending(&self) -> Option<(Seq, Vec<String>)> {
        let rebuild = &self.compactions.rebuild;
        rebuild.asked.clone().filter(|_| !rebuild.fed)
    }

    /// 喂进一条重读结果：是那次摘要请求在路上时来的、一个对一个的，记下来。读到的原文记着，载入以后交回去。
    pub(super) fn reread_fed(&mut self, input: &Input) {
        let Input::Reread { seen, files, .. } = input else {
            return;
        };
        let in_flight = self
            .compactions
            .summarizing
            .map(|(summarizing, _)| summarizing);
        let rebuild = &mut self.compactions.rebuild;
        let asked = rebuild.asked.as_ref().filter(|(asked, _)| *asked == *seen);
        if let Some((_, paths)) = asked {
            rebuild.fed = true;
            if in_flight == Some(*seen) && files.len() == paths.len() {
                rebuild.answered = Some(files.clone());
            }
        }
        for file in files {
            if let Reread::Read { blob, text } = file {
                rebuild.texts.insert(blob.clone(), text.clone());
            }
        }
    }

    /// 写下了一条压缩：照收到的重读结果查重读了哪些（见文件开头）。
    pub(super) fn rebuild_checked(&mut self, compacted: &ContextCompacted) {
        let seed = self.seed;
        let rebuild = std::mem::take(&mut self.compactions.rebuild);
        self.compactions.rebuild.texts = rebuild.texts.clone();
        self.compactions.rebuild.recalling = rebuild.recalling.clone();
        let numbers = random_policy(true)
            .compaction
            .and_then(|compaction| compaction.rebuild)
            .expect("随机测试的策略开着压后重建");
        let read: Vec<(ContentHash, &String)> = rebuild
            .answered
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|file| match file {
                Reread::Read { blob, text } => Some((blob.clone(), text)),
                _ => None,
            })
            .collect();
        assert!(
            compacted.restored.len() <= numbers.files,
            "种子 {seed}：重读了 {} 个，多过 {}",
            compacted.restored.len(),
            numbers.files
        );
        let mut rest = read.iter();
        let mut total = 0u64;
        for RestoredFile { blob, tokens, .. } in compacted.restored.iter().cloned() {
            let found = rest.by_ref().find(|(read, _)| *read == blob);
            let Some((_, text)) = found else {
                panic!(
                    "种子 {seed}：重读的 {blob} 不是照先后读到了的：{:?}",
                    rebuild.answered
                );
            };
            assert_eq!(
                tokens,
                crate::estimate::text(text),
                "种子 {seed}：tokens 照原文估"
            );
            assert!(tokens <= numbers.file_tokens, "种子 {seed}：单个超了");
            total += tokens;
        }
        assert!(total <= numbers.total, "种子 {seed}：合计超了");
        assert!(
            compacted
                .notes
                .contains(&format!("<retrieve {}/>", compacted.upto)),
            "种子 {seed}：代码写的几段没有取回指路：{:?}",
            compacted.notes
        );
    }

    /// 取回原文的回报（施工 6-9）：内核要的那几份，照记着的原文，没有的不交。没在要的，没有。
    pub(in super::super) fn recall_answer(&mut self) -> Option<Input> {
        let blobs = self.compactions.rebuild.recalling.take()?;
        self.seen_paths.insert("取回了原文");
        let texts = blobs
            .iter()
            .filter_map(|blob| {
                let text = self.compactions.rebuild.texts.get(blob)?;
                Some((blob.clone(), text.clone()))
            })
            .collect();
        Some(Input::Recalled { texts })
    }
}
