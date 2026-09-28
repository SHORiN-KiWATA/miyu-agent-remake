//! `miyu undo`、`miyu redo` 给人看的字（施工 4-7 下，样子是项目主人 2026-09-28 定的）。

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
            (Language::Chinese, Direction::Redo, Some(said), 0 | 1) => {
                format!("· 恢复「{said}」这一轮")
            }
            (Language::Chinese, Direction::Redo, Some(said), turns) => {
                format!("· 恢复「{said}」起的 {turns} 轮")
            }
            (Language::Chinese, Direction::Redo, None, _) => "· 恢复撤销的那一轮".to_string(),
            (Language::English, Direction::Undo, Some(said), 0 | 1) => {
                format!("· Undid the turn \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Undo, Some(said), turns) => {
                format!("· Undid {turns} turns from \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Undo, None, _) => "· Undid the last turn".to_string(),
            (Language::English, Direction::Redo, Some(said), 0 | 1) => {
                format!("· Redid the turn \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Redo, Some(said), turns) => {
                format!("· Redid {turns} turns from \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Redo, None, _) => "· Redid the undone turn".to_string(),
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
            (Language::Chinese, Direction::Undo) => "--- 她改完的",
            (Language::Chinese, Direction::Redo) => "--- 撤销以后的",
            (Language::English, Direction::Undo) => "--- as she left it",
            (Language::English, Direction::Redo) => "--- as undone",
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

    /// 撤销的最后一行。
    pub(crate) fn redo_hint(&self) -> &'static str {
        match self {
            Language::Chinese => "发下一句之前，可以用 miyu redo 恢复。",
            Language::English => "Until you say something else, miyu redo brings it back.",
        }
    }

    /// `miyu undo`、`miyu redo` 的 `--help` 里那一句。
    pub fn undo_about(&self, direction: Direction) -> &'static str {
        match (self, direction) {
            (Language::Chinese, Direction::Undo) => "撤掉当前会话的最后一轮，把她改过的文件改回去",
            (Language::Chinese, Direction::Redo) => "发下一句之前，恢复最近一次撤销",
            (Language::English, Direction::Undo) => {
                "Undo the last turn of the current session and restore the files she changed"
            }
            (Language::English, Direction::Redo) => {
                "Redo the latest undo, until you say something else"
            }
        }
    }

    /// 核心没在跑，又没设 key：不拉起（施工 4-9 再补一，照 `miyu ask` 的规矩）。
    pub(crate) fn undo_needs_key(&self) -> &'static str {
        match self {
            Language::Chinese => {
                "核心没在跑。先设 DEEPSEEK_API_KEY：没有 key 拉起的核心，之后的 miyu ask 也用不了"
            }
            Language::English => {
                "The core is not running. Set DEEPSEEK_API_KEY first: a core started without it cannot serve miyu ask later"
            }
        }
    }

    /// `--session` 的说明。
    pub fn undo_session_help(&self) -> &'static str {
        match self {
            Language::Chinese => "哪个会话；不写的是上一次 miyu ask 开的那个",
            Language::English => "Which session; the one the last miyu ask opened by default",
        }
    }
}
