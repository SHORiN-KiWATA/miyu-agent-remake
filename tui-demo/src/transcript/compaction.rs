//! 压缩那几行（蓝图 `tui.md`「正文」第 9 条，照 `miyu ask`）：进度一行原地刷新，压好了、失败了换成结果，
//! 暂停了自动压缩写一行红字；一轮结束时还在「正在压缩」的藏起来（被打断了，收尾会说）。

use std::time::Instant;

use super::{Kind, Progress, Transcript};
use crate::config::{CompactionMotion, Texts};
use crate::core::Compaction;
use crate::meter;
use crate::rng::Rng;

/// 内核在摘要请求调了工具、接着改走隔离式时，原话末尾写的（`docs/blueprint/compaction.md` 第三条第 7 条）。
const ISOLATING: &str = "trying again without tools";

impl Transcript {
    /// 收一样压缩的推送。
    pub(super) fn compaction(&mut self, push: Compaction, texts: &Texts) {
        let words = &texts.compaction;
        match push {
            // 流光、点、下面的进度条画的时候加（`ui/compaction_rows.rs`）。
            Compaction::Progress { written, expected } => {
                let count = words
                    .written
                    .replace("{written}", &meter::thousands(written));
                let text = format!("{}{count}", words.progress);
                // 同一次压缩接着记：进度条亮到哪、这次从哪一刻开始都留着。
                let progress = match self.compacting_progress() {
                    Some(mut p) => {
                        p.written = written;
                        p.expected = expected;
                        p
                    }
                    None => Progress::new(written, expected, Instant::now()),
                };
                self.compacting_line(Kind::Note, text, Some(progress));
            }
            Compaction::Done { before, after } => {
                let text = words
                    .done
                    .replace("{before}", &meter::short(before))
                    .replace("{after}", &meter::short(after));
                self.compacting_line(Kind::Note, text, None);
                self.compacting = None;
            }
            // 调了工具、接着改走隔离式（施工 6-6 下）：不是失败，灰色说一句，这次压缩接着来进度（另起一行）。
            Compaction::Failed { class, message }
                if class == "bad_summary" && message.ends_with(ISOLATING) =>
            {
                self.compacting_line(Kind::Note, words.isolating.clone(), None);
                self.compacting = None;
            }
            Compaction::Failed { class, message } => {
                let reason = if class == "bad_summary" && message.contains("called a tool") {
                    words.called_a_tool.clone()
                } else {
                    class_name(&class, texts)
                };
                self.compacting_line(Kind::Error, words.failed.replace("{reason}", &reason), None);
                self.compacting = None;
            }
            Compaction::Paused {
                reason,
                failures,
                entry,
            } => {
                let text = match (reason.as_str(), failures, entry) {
                    ("failures", Some(n), _) => {
                        words.paused_failures.replace("{n}", &n.to_string())
                    }
                    ("too_large", _, Some(entry)) => words
                        .paused_too_large
                        .replace("{entry}", &entry.to_string()),
                    _ => words.paused.clone(),
                };
                self.note(Kind::Error, text);
            }
        }
    }

    /// 一轮结束：还在「正在压缩」的那一行藏起来，这一轮的收尾会说。
    pub(super) fn drop_compacting(&mut self) {
        if let Some(entry) = self.compacting.take().and_then(|i| self.entries.get_mut(i)) {
            entry.hidden = true;
        }
    }

    /// 每一帧追一下正在压缩那一行的进度条（`app` 的 `tick` 叫）。
    pub fn climb(&mut self, now: Instant, width: usize, look: &CompactionMotion, rng: &mut Rng) {
        let entry = self.compacting.and_then(|i| self.entries.get_mut(i));
        if let Some(progress) = entry.and_then(|e| e.progress.as_mut()) {
            progress.climb(now, width, look, rng);
        }
    }

    /// 正在压缩那一行现在的进度。
    fn compacting_progress(&self) -> Option<Progress> {
        let entry = self.compacting.and_then(|i| self.entries.get(i))?;
        entry.progress.clone()
    }

    /// 写压缩那一行：有正在压的那一行就原地换掉，没有就在正文末尾另起一行。`progress` 是还在压时的进度。
    fn compacting_line(&mut self, kind: Kind, text: String, progress: Option<Progress>) {
        if self.compacting.is_none() {
            self.note(kind.clone(), String::new());
            self.compacting = Some(self.entries.len() - 1);
        }
        if let Some(entry) = self.compacting.and_then(|i| self.entries.get_mut(i)) {
            entry.kind = kind;
            entry.text = text;
            entry.progress = progress;
        }
    }
}

/// 出错的分类写成人话（`text/zh.json` 的 `error_classes`）；认不得的照原样。
pub(super) fn class_name(class: &str, texts: &Texts) -> String {
    texts
        .error_classes
        .get(class)
        .cloned()
        .unwrap_or_else(|| class.to_string())
}
