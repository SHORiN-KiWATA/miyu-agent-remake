//! 给人看的话跟着界面语言（`docs/designs/22-命令行.md` 第二节）：握手以前照系统的语言，`zh` 开头的说中文，别的说
//! 英文；握手以后照核心回的 `language`，它照这个人的 `ui.language` 算（施工 8-2）。界面的字先写在这里，做界面
//! 语言的那一步挪进资源文件（`00-设计理念.md` 第六节）。

use crate::ask::usage_line;

mod agents;
mod config;
mod config_write;
mod harness;
mod login;
mod memory;
mod pkg;
mod sandbox;
mod setup;
mod undo;

pub(crate) use pkg::Checked;

/// 界面语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// 中文。
    Chinese,
    /// 英文。
    English,
}

/// 一步的结果里头自己写的几个词（施工 4-5 下）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Word {
    /// 出错了：标红，有原因的后面跟原因。
    Failed,
    /// 被拒了，没做：标红，有原因的后面跟原因。
    Denied,
    /// 打断了：没有说法时写。
    Cancelled,
    /// 跳过了：没有说法时写。
    Skipped,
}

/// 握手以前的界面语言：照系统的语言（`miyu_store::env::locale`：`LC_ALL`、`LC_MESSAGES`、`LANG`，都没设的看系统设置，
/// 施工 8-2），`zh` 开头的说中文，别的说英文。握手以后照核心回的 `language`（[`Language::from_code`]）。
pub fn current() -> Language {
    of_locale(miyu_store::env::locale().as_deref())
}

/// 照系统的语言 `locale` 挑：`zh` 开头的是中文，别的、没有的是英文。
pub fn of_locale(locale: Option<&str>) -> Language {
    match locale {
        Some(value) if value.starts_with("zh") => Language::Chinese,
        _ => Language::English,
    }
}

impl Language {
    /// 核心握手时回的 `language`（`zh`、`en`、`ja`）换成命令行的界面语言（施工 8-2）：命令行自己的字只有中文、英文，
    /// `ja` 的照英文。认不出的是空的。
    pub fn from_code(code: &str) -> Option<Language> {
        match code {
            "zh" => Some(Language::Chinese),
            "en" | "ja" => Some(Language::English),
            _ => None,
        }
    }

    /// 握手时报给核心的语言：核心的拒绝照它说。
    pub fn locale(&self) -> &'static str {
        match self {
            Language::Chinese => "zh-CN",
            Language::English => "en",
        }
    }

    /// 给人看的字读哪一份：`human/<它>.json`（施工 4-5 下）。
    pub fn code(&self) -> &'static str {
        match self {
            Language::Chinese => "zh",
            Language::English => "en",
        }
    }

    /// 一步的结果里头自己写的词。
    pub(crate) fn word(&self, word: Word) -> &'static str {
        let (chinese, english) = match word {
            Word::Failed => ("出错", "failed"),
            Word::Denied => ("没做", "not done"),
            Word::Cancelled => ("打断了", "interrupted"),
            Word::Skipped => ("已跳过", "skipped"),
        };
        match self {
            Language::Chinese => chinese,
            Language::English => english,
        }
    }

    /// 有 `steps` 步因为要确认没做（`22-命令行.md` O3，施工 4-9）：红的「没做」前面、后面的两段。
    pub(crate) fn unattended(&self, steps: u64) -> (String, &'static str) {
        match (self, steps) {
            (Language::Chinese, _) => (format!("· {steps} 步"), "：要你确认，miyu ask 里确认不了"),
            (Language::English, 1) => (
                "· 1 step ".to_string(),
                ": it needs your approval, which cannot be given in miyu ask",
            ),
            (Language::English, _) => (
                format!("· {steps} steps "),
                ": they need your approval, which cannot be given in miyu ask",
            ),
        }
    }

    /// 「出错」「没做」和原因之间。
    pub(crate) fn colon(&self) -> &'static str {
        match self {
            Language::Chinese => "：",
            Language::English => ": ",
        }
    }

    /// 接着说的会话在别处干活（施工 9-7 上）：工作区是会话的，`used` 是它在的目录。
    pub(crate) fn elsewhere(&self, used: &str) -> String {
        match self {
            Language::Chinese => format!("· 这个会话在 {used} 里干活"),
            Language::English => format!("· This session works in {used}"),
        }
    }

    /// 工作目录太宽，核心退回了账号的工作区：`given` 是敲命令时的目录，`used` 是实际干活的。
    pub(crate) fn moved(&self, given: &str, used: &str) -> String {
        match self {
            Language::Chinese => format!("· 目录太宽（{given}），这次在 {used} 里干活"),
            Language::English => {
                format!("· Working directory too wide ({given}), using {used} this time")
            }
        }
    }

    /// 没有这个子命令。
    pub fn no_such_command(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("没有 {name} 这个子命令。想和 AI 对话，用 miyu ask \"…\""),
            Language::English => {
                format!("There is no {name} command. To talk to the AI, use miyu ask \"…\"")
            }
        }
    }

    /// 没有可用的模型（施工 8-6；施工 8-11 起指向 `miyu setup`，`models.md`「给人看的字」）。
    pub fn no_model(&self) -> String {
        match self {
            Language::Chinese => "没有可用的模型：还没配。运行 miyu setup。".to_string(),
            Language::English => {
                "No model is available: none is set up. Run miyu setup.".to_string()
            }
        }
    }

    /// `--continue` 找不到可以接着说的会话。
    pub fn no_oneshot(&self) -> String {
        match self {
            Language::Chinese => "还没有 miyu ask 开过的会话".to_string(),
            Language::English => "No session opened by miyu ask yet".to_string(),
        }
    }

    /// 出错了：哪一类，原话。
    pub fn failed(&self, class: &str, message: &str) -> String {
        let (kind, said) = (self.class(class), message.trim());
        match (self, said.is_empty()) {
            (Language::Chinese, true) => format!("出错了：{kind}"),
            (Language::Chinese, false) => format!("出错了：{kind}：{said}"),
            (Language::English, true) => format!("Error: {kind}"),
            (Language::English, false) => format!("Error: {kind}: {said}"),
        }
    }

    /// 出错的分类怎么说（`model.called` 的 `error.class`）。
    /// 出错的分类说成给人看的（施工 6-3 下：压缩失败那一行也用）。
    pub(crate) fn class_name(&self, class: &str) -> &'static str {
        self.class(class)
    }

    fn class(&self, class: &str) -> &'static str {
        let (chinese, english) = match class {
            "retryable" => ("临时错误", "temporary error"),
            "rate_limited" => ("请求受限", "rate limited"),
            "context_too_long" => ("上下文太长", "context too long"),
            "auth" => ("认证失败", "authentication failed"),
            "content_policy" => ("内容被拦截", "blocked by content policy"),
            "bad_stream" => ("响应流异常", "malformed response stream"),
            "empty_reply" => ("响应为空", "empty response"),
            "bad_summary" => ("摘要提取失败", "summary extraction failed"),
            "compaction_paused" => ("自动压缩已暂停", "auto-compaction paused"),
            "no_model" => ("没有可用的模型", "no model available"),
            "cooling" => ("模型冷却中", "models cooling down"),
            _ => ("模型错误", "model error"),
        };
        match self {
            Language::Chinese => chinese,
            Language::English => english,
        }
    }

    /// 被打断了。
    pub fn interrupted(&self) -> String {
        match self {
            Language::Chinese => "打断了".to_string(),
            Language::English => "Interrupted".to_string(),
        }
    }

    /// 这一轮没走完：步数用完、核心崩了或者重启了。
    pub fn unfinished(&self, reason: &str) -> String {
        match self {
            Language::Chinese => format!("这一轮没走完：{reason}"),
            Language::English => format!("The turn did not finish: {reason}"),
        }
    }

    /// 核心断开了。
    pub fn disconnected(&self) -> String {
        match self {
            Language::Chinese => "核心断开了".to_string(),
            Language::English => "The core went away".to_string(),
        }
    }

    /// 附件传不上（施工 3-9 三补）：哪个文件，核心照握手时报的语言说的原因。
    pub(crate) fn not_attached(&self, file: &str, reason: &str) -> String {
        match self {
            Language::Chinese => format!("附不上 {file}：{reason}"),
            Language::English => format!("Cannot attach {file}: {reason}"),
        }
    }

    /// 核心拒绝了：它的原话已经照握手时报的语言说了。
    pub fn refused(&self, reason: &str) -> String {
        reason.to_string()
    }

    /// 问完那一行用量。
    pub(crate) fn usage(&self, sum: &crate::ask::Sum) -> String {
        usage_line(self, sum)
    }
}
