//! 压后重建（`docs/blueprint/compaction.md` 第八、九条，施工 6-5）：检查点里代码写的几段（读过、改过的文件清单，
//! 取回指路，太大没重读的），和最近读过、改过的几个文件的原文。
//!
//! 内核不碰磁盘：发摘要请求之前交出要重读的候选（[`Session::reread_paths`]），执行器读完、存成 blob 送回来，记在
//! 那次摘要请求上；取到摘要以后照先后挑（[`Session::rebuild`]）。

use std::collections::{BTreeMap, BTreeSet};

use super::Session;
use super::input::Reread;
use super::policy::{Notes, Rebuild};
use crate::estimate;
use crate::event::{Body, CompactTrigger, ContextCompacted, Effect, Event, RestoredFile};
use crate::id::{ContentHash, Seq};
use crate::origin::By;
use crate::template::Template;
use crate::time::Timestamp;

/// 清单里最多写几个文件，多的写还有几个。
const LISTED: usize = 30;
/// 交给执行器的单个上限：token 折成字节，和本地估算一样一个 token 算 4 个字节。
const BYTES_PER_TOKEN: u64 = 4;

/// 挑好的：代码写的几段、重读的文件、它们的原文（照 blob 找）。
#[derive(Debug, Default)]
pub(super) struct Rebuilt {
    pub(super) notes: String,
    pub(super) restored: Vec<RestoredFile>,
    pub(super) texts: BTreeMap<ContentHash, String>,
}

impl Session {
    /// 压后重建的数：策略里有、窗口够的才有（第九条第 2 条）。
    fn rebuild_numbers(&self) -> Option<Rebuild> {
        let rebuild = self.policy.compaction.as_ref()?.rebuild?;
        self.policy.notes.as_ref()?;
        let window = self.limits.as_ref()?.window?;
        (window >= rebuild.min_window).then_some(rebuild)
    }

    /// 发第 `upto` 条的摘要请求之前，交给执行器重读的候选（第九条第 1 条）：被替代的那一段里读过、改过的文件，最近
    /// 的在前、去重；尾巴里读过、改过的跳过；最多 [`Rebuild::candidates`] 个。重读不了的（策略里没有、窗口不够）没有。
    pub(super) fn reread_paths(&self, upto: Seq) -> Vec<String> {
        let Some(rebuild) = self.rebuild_numbers() else {
            return Vec::new();
        };
        let tail: BTreeSet<String> = self.touched(|seq| seq > upto).into_iter().collect();
        self.touched(|seq| seq <= upto)
            .into_iter()
            .filter(|path| !tail.contains(path))
            .take(rebuild.candidates)
            .collect()
    }

    /// 交给执行器的单个上限，字节。
    pub(super) fn reread_limit(&self) -> u64 {
        self.rebuild_numbers().map_or(0, |rebuild| {
            rebuild.file_tokens.saturating_mul(BYTES_PER_TOKEN)
        })
    }

    /// 取到第 `upto` 条的摘要以后挑（第九条第 3 条）：重读到的照先后，单个不超过 [`Rebuild::file_tokens`]、合计不超过
    /// [`Rebuild::total`]、加上以后压完的整份请求不超过压缩线的一半，挑满 [`Rebuild::files`] 个为止；太大的、放不下的
    /// 进清单。`reread` 和 `paths` 一个对一个（对不上的在收的时候就不收，[`super::compaction::Compacting`]），没收到（执行器
    /// 没做）的照没有候选写。策略里没有几段的模板的，什么都不写。
    pub(super) fn rebuild(
        &self,
        at: Timestamp,
        upto: Seq,
        summary: &str,
        paths: &[String],
        reread: Option<&[Reread]>,
    ) -> Rebuilt {
        let Some(notes) = self.policy.notes.as_ref() else {
            return Rebuilt::default();
        };
        let listed: Vec<String> = self
            .touched(|seq| seq <= upto)
            .iter()
            .map(|path| self.shown(path))
            .collect();
        let head = files_and_retrieve(notes, &listed, upto);
        let mut rebuilt = Rebuilt::default();
        let mut too_large = Vec::new();
        let picked = reread
            .zip(self.rebuild_numbers())
            .zip(self.room(at, upto, summary, &head));
        if let Some(((reread, rebuild), room)) = picked {
            let mut total = 0u64;
            for (path, file) in paths.iter().zip(reread) {
                let shown = self.shown(path);
                match file {
                    _ if rebuilt.restored.len() == rebuild.files => break,
                    Reread::Read { blob, text } => {
                        let tokens = estimate::text(text);
                        let fits = tokens <= rebuild.file_tokens
                            && total.saturating_add(tokens) <= rebuild.total.min(room);
                        if !fits {
                            too_large.push(shown);
                            continue;
                        }
                        total = total.saturating_add(tokens);
                        rebuilt.restored.push(RestoredFile {
                            path: shown,
                            blob: blob.clone(),
                            tokens,
                        });
                        rebuilt.texts.insert(blob.clone(), text.clone());
                    }
                    Reread::TooLarge => too_large.push(shown),
                    Reread::Unreadable => {}
                }
            }
        }
        rebuilt.notes = head;
        if !too_large.is_empty() {
            rebuilt
                .notes
                .push_str(&say(&notes.too_large, &[("files", &too_large.join(", "))]));
        }
        rebuilt
    }

    /// 压完的整份请求还能放多少重读的原文：压缩线的一半，减去不带重读的文件时压完的整份请求的估算。没交限额的，没有。
    fn room(&self, at: Timestamp, upto: Seq, summary: &str, notes: &str) -> Option<u64> {
        let line = self.line()?;
        let mut trial = self.history.clone();
        trial.append(Event {
            seq: self.ledger.next_seq(),
            at,
            turn: None,
            by: By::Kernel,
            cause: None,
            body: Body::ContextCompacted(ContextCompacted {
                upto,
                summary: summary.to_string(),
                trigger: Some(CompactTrigger::Auto),
                notes: notes.to_string(),
                restored: Vec::new(),
            }),
        });
        let request = self.policy.assembler.assemble(&trial);
        let base = self.used_in(&trial, &request)?;
        Some((line / 2).saturating_sub(base))
    }

    /// 有效历史里、序号合 `keep` 的工具结果报过的读过、改过的文件，真实的位置，最近的在前、去重。
    fn touched(&self, keep: impl Fn(Seq) -> bool) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut paths = Vec::new();
        for event in self.history.events().iter().rev() {
            let Body::ToolResult(result) = &event.body else {
                continue;
            };
            if !keep(event.seq) {
                continue;
            }
            for effect in result.effects.iter().rev() {
                let path = match effect {
                    Effect::FileRead(read) => &read.path,
                    Effect::FileChanged(changed) => &changed.path,
                    _ => continue,
                };
                if seen.insert(path.clone()) {
                    paths.push(path.clone());
                }
            }
        }
        paths
    }

    /// 清单、包装里的写法：在会话的工作目录里的写相对的，别的写绝对的。
    fn shown(&self, path: &str) -> String {
        let cwd = self.environment.cwd.trim_end_matches(['/', '\\']);
        match path.strip_prefix(cwd) {
            Some("") => ".".to_string(),
            Some(rest) if rest.starts_with(['/', '\\']) && !cwd.is_empty() => rest[1..].to_string(),
            _ => path.to_string(),
        }
    }
}

/// 清单（最多 [`LISTED`] 个，多的写还有几个）和取回指路。清单一个一行，行照原样写：模板换字段时会把换行转义掉。
fn files_and_retrieve(notes: &Notes, listed: &[String], upto: Seq) -> String {
    let mut text = String::new();
    if !listed.is_empty() {
        text.push_str(&say(&notes.files, &[]));
        for path in listed.iter().take(LISTED) {
            text.push_str(&format!("- {path}\n"));
        }
        if let Some(more) = listed.len().checked_sub(LISTED).filter(|more| *more > 0) {
            text.push_str(&say(&notes.files_more, &[("count", &more.to_string())]));
        }
    }
    text.push_str(&say(&notes.retrieve, &[("upto", &upto.to_string())]));
    text
}

/// 照模板换进字段；模板造的时候试换过，缺了字段的不会有，缺了也只是写空。
fn say(template: &Template, fields: &[(&str, &str)]) -> String {
    let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
    template.render(&fields).unwrap_or_default()
}

#[cfg(test)]
mod tests;
