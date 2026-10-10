//! 压缩那几行（蓝图 `tui.md`「正文」第 9 条，照 `miyu ask`）：进度一行原地刷新，压好了、失败了换成结果，
//! 暂停了自动压缩写一行红字；一轮结束时还在「正在压缩」的藏起来（被打断了，收尾会说）。

use std::time::Instant;

use super::climb::Finish;
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
            Compaction::Progress {
                written,
                expected,
                manual,
            } => {
                self.settle_filling();
                // 自动压缩停下来等：行首绿点、不画进度条；手动的（`/compact`）照旧有（「正文」第 9 条，2026-10-10 项目
                // 主人）。旧核心不带 `trigger`：这个头自己发了 `/compact` 的算手动。
                let manual = manual.unwrap_or(self.manual_compaction);
                let expected = expected.filter(|_| manual);
                let count = words
                    .written
                    .replace("{written}", &meter::thousands(written));
                let text = format!("{}{count}", words.progress);
                // 同一次压缩接着记：进度条亮到哪、这次从哪一刻开始都留着。字数变小是重来了（核心 6-11 下：提前压的那一次
                // 出错、改当场压），从零画。
                let progress = match self.compacting_progress() {
                    Some(mut p) if written >= p.written => {
                        p.written = written;
                        p.expected = expected;
                        p
                    }
                    _ => Progress::new(written, expected, Instant::now()),
                };
                self.compacting_line(Kind::Note, text, Some(progress));
                if !manual && let Some(entry) = self.compacting_mut() {
                    entry.mark = Some(words.done_mark.clone());
                }
            }
            // 有进度条的先走满、停一下再换（`climb`）；没有条的当场换。
            // 提前在后台开始压：正文不写（提示由 App 弹，「正文」第 9 条）。
            Compaction::Started => {}
            // 提前在后台压好、没停下来等就换上了：正文不写（同上）。
            Compaction::Done { prepared: true, .. } if self.compacting.is_none() => {
                self.manual_compaction = false;
            }
            Compaction::Done { before, after, .. } => {
                self.manual_compaction = false;
                let text = words
                    .done
                    .replace("{before}", &meter::short(before))
                    .replace("{after}", &meter::short(after));
                let mark = words.done_mark.clone();
                let filling = self.compacting_mut().and_then(|e| e.progress.as_mut());
                match filling {
                    Some(progress) if progress.expected.is_some() => {
                        progress.done(Instant::now(), text, mark);
                    }
                    _ => {
                        self.compacting_line(Kind::Note, text, None);
                        if let Some(entry) = self.compacting_mut() {
                            entry.mark = Some(mark);
                        }
                        self.compacting = None;
                    }
                }
            }
            // 调了工具、接着改走隔离式（施工 6-6 下）：不是失败，灰色说一句，这次压缩接着来进度（另起一行）。
            Compaction::Failed(error)
                if error.class == "bad_summary" && error.message.ends_with(ISOLATING) =>
            {
                self.compacting_line(Kind::Note, words.isolating.clone(), None);
                self.compacting = None;
            }
            Compaction::Failed(error) => {
                self.manual_compaction = false;
                let reason =
                    if error.class == "bad_summary" && error.message.contains("called a tool") {
                        words.called_a_tool.clone()
                    } else {
                        super::failure::reason(&error, texts)
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
    /// 一轮结束：还在「正在压缩」的那一行藏起来，这一轮的收尾会说。压好了、条还在走满的留着接着走
    /// （手动压缩那一轮压好就结束），走满了由 `climb` 换成结果。
    pub(super) fn drop_compacting(&mut self) {
        self.manual_compaction = false;
        let filling = self
            .compacting_mut()
            .is_some_and(|e| e.progress.as_ref().is_some_and(|p| p.finish.is_some()));
        if filling {
            return;
        }
        if let Some(entry) = self.compacting.take().and_then(|i| self.entries.get_mut(i)) {
            entry.hidden = true;
        }
    }

    /// 走满以前又来了一次压缩：先把走着的那一行换成结果，新的另起一行。
    fn settle_filling(&mut self) {
        let Some(entry) = self.compacting_mut() else {
            return;
        };
        if let Some(finish) = entry.progress.as_ref().and_then(|p| p.finish.clone()) {
            settle(entry, finish);
            self.compacting = None;
        }
    }

    /// 手动压缩那一轮完了：`tail`（用时和用量）接在压好了那一行后面；还在走满的接在它记着的结果上。
    /// 没有压好了那一行的交回假，照常写收尾行。
    pub(super) fn append_to_result(&mut self, tail: &str) -> bool {
        if let Some(finish) = self
            .compacting_mut()
            .and_then(|e| e.progress.as_mut())
            .and_then(|p| p.finish.as_mut())
        {
            finish.text.push_str(tail);
            return true;
        }
        match self.entries.iter_mut().rev().find(|e| e.mark.is_some()) {
            Some(entry) => {
                entry.text.push_str(tail);
                true
            }
            None => false,
        }
    }

    fn compacting_mut(&mut self) -> Option<&mut super::Entry> {
        self.compacting.and_then(|i| self.entries.get_mut(i))
    }

    /// 每一帧追一下正在压缩那一行的进度条（`app` 的 `tick` 叫）。
    pub fn climb(&mut self, now: Instant, width: usize, look: &CompactionMotion, rng: &mut Rng) {
        let Some(entry) = self.compacting_mut() else {
            return;
        };
        let Some(progress) = entry.progress.as_mut() else {
            return;
        };
        progress.climb(now, width, look, rng);
        // 走满、停够了：换成结果。
        if progress.filled(now, width, look)
            && let Some(finish) = progress.finish.clone()
        {
            settle(entry, finish);
            self.compacting = None;
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

    /// 清空了（`context.compacted` 的 `trigger` 是 `clear`）：正文一行绿点「上下文已清空」，这一轮不另起收尾行
    /// （「正文」第 9 条）。
    pub(super) fn cleared(&mut self, texts: &Texts) {
        let words = &texts.compaction;
        // 归到清空那一轮：撤掉那一轮时跟着藏起来（`note` 不归到哪一轮）。
        self.push(Kind::Note, words.cleared.clone());
        if let Some(entry) = self.entries.last_mut() {
            entry.mark = Some(words.done_mark.clone());
        }
        self.cleared = true;
        // 上下文用量清零，下一次请求再照实际的写（「正文」第 9 条）。
        self.context = 0;
    }
}

/// 压缩那一行换成压好了的结果：绿色记号，字暗，不再转、不再画条。
fn settle(entry: &mut super::Entry, finish: Finish) {
    entry.kind = Kind::Note;
    entry.text = finish.text;
    entry.mark = Some(finish.mark);
    entry.progress = None;
}
