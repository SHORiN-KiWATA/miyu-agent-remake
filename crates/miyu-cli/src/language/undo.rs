//! `miyu undo`、`miyu restore` 给人看的字（施工 4-7 下，样子是项目主人 2026-09-28 定的）；`miyu redo` 的第一行（施工 4-7 再补）。

use super::Language;
use crate::undo::Direction;

impl Language {
    /// 第一行：撤的（恢复的）是哪一轮。`said` 是那一轮人说的那句话，没有的写「最后一轮」；几轮的写几轮。
    pub(crate) fn undo_header(
        &self,
        direction: Direction,
        said: Option<&str>,
        turns: usize,
    ) -> String {
        match (self, direction, said, turns) {
            (Language::Chinese, Direction::Undo, Some(said), 0 | 1) => {
                format!("· 撤销「{said}」这一轮")
            }
            (Language::Chinese, Direction::Undo, Some(said), turns) => {
                format!("· 撤销「{said}」起的 {turns} 轮")
            }
            (Language::Chinese, Direction::Undo, None, _) => "· 撤销最后一轮".to_string(),
            (Language::Chinese, Direction::Restore, Some(said), 0 | 1) => {
                format!("· 恢复「{said}」这一轮")
            }
            (Language::Chinese, Direction::Restore, Some(said), turns) => {
                format!("· 恢复「{said}」起的 {turns} 轮")
            }
            (Language::Chinese, Direction::Restore, None, _) => "· 恢复撤销的那一轮".to_string(),
            (Language::English, Direction::Undo, Some(said), 0 | 1) => {
                format!("· Undid the turn \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Undo, Some(said), turns) => {
                format!("· Undid {turns} turns from \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Undo, None, _) => "· Undid the last turn".to_string(),
            (Language::English, Direction::Restore, Some(said), 0 | 1) => {
                format!("· Restored the turn \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Restore, Some(said), turns) => {
                format!("· Restored {turns} turns from \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Restore, None, _) => {
                "· Restored the undone turn".to_string()
            }
        }
    }

    /// `miyu redo` 的第一行（施工 4-7 再补，`docs/blueprint/cli/redo.md`）：照撤销一轮的那一行，接「，重新做」。`said` 是撤掉的
    /// 那一轮原来那一句，没有的写「最后一轮」。
    pub(crate) fn redo_header(&self, said: Option<&str>) -> String {
        match (self, said) {
            (Language::Chinese, Some(said)) => format!("· 撤销「{said}」这一轮，重新做"),
            (Language::Chinese, None) => "· 撤销最后一轮，重新做".to_string(),
            (Language::English, Some(said)) => {
                format!("· Undid the turn \u{201c}{said}\u{201d}, redoing it")
            }
            (Language::English, None) => "· Undid the last turn, redoing it".to_string(),
        }
    }

    /// 改回的一步做了什么：写回、移进回收站、从回收站移回来。不认识的照原样写。
    pub(crate) fn undo_verb<'a>(&self, action: &'a str) -> &'a str {
        match (self, action) {
            (Language::Chinese, "write") => "改回",
            (Language::Chinese, "trash") => "删掉",
            (Language::Chinese, "untrash") => "移回",
            (Language::English, "write") => "Restored",
            (Language::English, "trash") => "Removed",
            (Language::English, "untrash") => "Put back",
            (_, other) => other,
        }
    }

    /// 移进了回收站的，跟在箭头后面的那一句。
    pub(crate) fn undo_trashed(&self) -> &'static str {
        match self {
            Language::Chinese => "移进了回收站",
            Language::English => "moved to the trash",
        }
    }

    /// 没动：红的那个词。
    pub(crate) fn untouched(&self) -> &'static str {
        match self {
            Language::Chinese => "没动",
            Language::English => "left alone",
        }
    }

    /// 没动的原因，照结局；不认识的结局没有原因。
    pub(crate) fn untouched_because(&self, outcome: &str) -> Option<&'static str> {
        Some(match (self, outcome) {
            (Language::Chinese, "changed") => "之后又被改过",
            (Language::Chinese, "missing") => "文件没了",
            (Language::Chinese, "occupied") => "原处有了别的",
            (Language::Chinese, "gone") => "回收站里已经没有了",
            (Language::Chinese, "unsaved") => "改前的内容当时没存下来",
            (Language::Chinese, "unavailable") => "回收站收不了",
            (Language::English, "changed") => "changed since",
            (Language::English, "missing") => "the file is gone",
            (Language::English, "occupied") => "something else is there now",
            (Language::English, "gone") => "it is no longer in the trash",
            (Language::English, "unsaved") => "the earlier content was not saved",
            (Language::English, "unavailable") => "the trash cannot take it",
            _ => return None,
        })
    }

    /// 差异的头一行：对照的是什么。撤销时是她改完的，恢复时是撤销以后的。
    pub(crate) fn diff_then(&self, direction: Direction) -> &'static str {
        match (self, direction) {
            (Language::Chinese, Direction::Undo) => "--- AI 改完的",
            (Language::Chinese, Direction::Restore) => "--- 撤销以后的",
            (Language::English, Direction::Undo) => "--- as the AI left it",
            (Language::English, Direction::Restore) => "--- as undone",
        }
    }

    /// 差异的第二行：现在的。
    pub(crate) fn diff_now(&self) -> &'static str {
        match self {
            Language::Chinese => "+++ 现在",
            Language::English => "+++ now",
        }
    }

    /// 差异没印完的，说还有几行。
    pub(crate) fn diff_more(&self, lines: u64) -> String {
        match self {
            Language::Chinese => format!("还有 {lines} 行"),
            Language::English if lines == 1 => "1 more line".to_string(),
            Language::English => format!("{lines} more lines"),
        }
    }

    /// 撤掉的几轮执行过命令的那一句（`10-自带软件.md` 第七节：经过 shell 的改动撤不回，界面上写明）。
    pub(crate) fn commands_note(&self, commands: u64, turns: usize) -> String {
        match (self, turns) {
            (Language::Chinese, 0 | 1) => {
                format!("· 这一轮执行过 {commands} 条命令：命令改的文件撤不回")
            }
            (Language::Chinese, _) => {
                format!("· 这几轮执行过 {commands} 条命令：命令改的文件撤不回")
            }
            (Language::English, _) if commands == 1 => {
                "· 1 command ran: files it changed cannot be undone".to_string()
            }
            (Language::English, _) => {
                format!("· {commands} commands ran: files they changed cannot be undone")
            }
        }
    }

    /// 撤掉了压缩的那一句（施工 6-9，`compaction.md`「给人看的字」）：撤掉一次、几次都是这一句（2026-09-29 项目主人定）。
    pub(crate) fn compaction_undone(&self) -> &'static str {
        match self {
            Language::Chinese => "· 撤掉了压缩，上下文回到了压缩前",
            Language::English => "· Undid the compaction; the context is back to how it was before",
        }
    }

    /// 撤掉了清空的那一句（施工 6-8 补，`compaction.md` 第十四条）：撤掉一次、几次都是这一句（2026-09-30 项目主人定）。
    pub(crate) fn clear_undone(&self) -> &'static str {
        match self {
            Language::Chinese => "· 撤掉了清空，上下文回到了清空以前",
            Language::English => "· Undid the clear; the context is back to before it.",
        }
    }

    /// 撤销停掉了那几轮派出去的任务的那一句（施工 7-8，`agents.md` 第七条第 1 条）：`count` 是停掉了几个。
    pub(crate) fn jobs_stopped(&self, count: usize) -> String {
        match (self, count) {
            (Language::Chinese, count) => format!("· 停掉了 {count} 个任务"),
            (Language::English, 1) => "· Stopped 1 job".to_string(),
            (Language::English, count) => format!("· Stopped {count} jobs"),
        }
    }

    /// 撤销的最后一行。
    pub(crate) fn restore_hint(&self) -> &'static str {
        match self {
            Language::Chinese => "发下一句之前，可以用 miyu restore 恢复。",
            Language::English => "Until you say something else, miyu restore brings it back.",
        }
    }
}
