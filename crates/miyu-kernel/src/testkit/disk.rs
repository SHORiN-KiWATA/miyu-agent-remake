//! 替身的「磁盘」（施工 6-5）：压后重建要重读的文件。测试照 [`Stage::disk`] 放进去；替身回「重读」照它回，读到的记成
//! blob；回「取回原文」照存过的 blob 交回原文，回「读回日志」照追加过的事件交回，和执行器一样（施工 6-9）。

use std::collections::BTreeMap;

use super::Stage;
use crate::id::{ContentHash, Seq};
use crate::session::{Input, Reread};

/// 替身的「磁盘」：文件、存过的 blob、交过来的每一次重读、读回日志、取回原文。
#[derive(Debug, Default)]
pub(super) struct Disk {
    files: BTreeMap<String, String>,
    blobs: BTreeMap<ContentHash, String>,
    rereads: Vec<(Seq, Vec<String>, u64)>,
    read_backs: Vec<Seq>,
    recalls: Vec<Vec<ContentHash>>,
}

impl Stage {
    /// 「磁盘」上放一个文件：真实的位置 `path`，内容 `text`。放过的再放是改了它。
    pub fn disk(&mut self, path: &str, text: &str) {
        self.disk.files.insert(path.to_string(), text.to_string());
    }

    /// 交过来的每一次重读，照先后：哪一次摘要请求、要读哪些、单个上限。
    pub fn rereads(&self) -> &[(Seq, Vec<String>, u64)] {
        &self.disk.rereads
    }

    /// 交过来的每一次读回日志，照先后：从第几条起（施工 6-9）。
    pub fn read_backs(&self) -> &[Seq] {
        &self.disk.read_backs
    }

    /// 交过来的每一次取回原文，照先后：要的 blob（施工 6-9）。
    pub fn recalls(&self) -> &[Vec<ContentHash>] {
        &self.disk.recalls
    }

    /// 照最近那一次重读再送一回结果 `files`，像执行器送晚了、送错了（施工 6-5）。
    pub fn answer_reread(&mut self, files: Vec<Reread>) {
        let Some((seen, _, _)) = self.disk.rereads.last().cloned() else {
            return;
        };
        let at = self.tick();
        self.run(Input::Reread { at, seen, files });
    }

    /// 回「重读」：「磁盘」上有的，超过上限的是太大，别的读到了、记成 blob；没有的读不到。
    pub(super) fn reread(&mut self, seen: Seq, paths: Vec<String>, limit: u64) -> Vec<Input> {
        let files = paths
            .iter()
            .map(|path| match self.disk.files.get(path) {
                Some(text) if u64::try_from(text.len()).unwrap_or(u64::MAX) > limit => {
                    Reread::TooLarge
                }
                Some(text) => {
                    let blob = ContentHash::of(text.as_bytes());
                    self.disk.blobs.insert(blob.clone(), text.clone());
                    Reread::Read {
                        blob,
                        text: text.clone(),
                    }
                }
                None => Reread::Unreadable,
            })
            .collect();
        self.disk.rereads.push((seen, paths, limit));
        vec![Input::Reread {
            at: self.tick(),
            seen,
            files,
        }]
    }

    /// 回「取回原文」（施工 6-9）：照存过的 blob 找，没有的不交。
    pub(super) fn recall(&mut self, blobs: Vec<ContentHash>) -> Vec<Input> {
        let texts: BTreeMap<ContentHash, String> = blobs
            .iter()
            .filter_map(|blob| Some((blob.clone(), self.disk.blobs.get(blob)?.clone())))
            .collect();
        self.disk.recalls.push(blobs);
        vec![Input::Recalled { texts }]
    }

    /// 回「读回日志」（施工 6-9）：「磁盘」上第 `from` 条起的事件。
    pub(super) fn read_back(&mut self, from: Seq) -> Vec<Input> {
        let events = self
            .log
            .iter()
            .filter(|event| event.seq >= from)
            .cloned()
            .collect();
        self.disk.read_backs.push(from);
        vec![Input::ReadBack {
            at: self.tick(),
            from,
            events,
        }]
    }
}
