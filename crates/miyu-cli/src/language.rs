//! 给人看的话跟着界面语言（`docs/designs/22-命令行.md` 第二节）：照 `LC_ALL`、`LC_MESSAGES`、`LANG`，`zh`
//! 开头的说中文，别的说英文。配置系统做出来以后照这个人的界面语言设置；界面的字先写在这里，做界面
//! 语言的那一步挪进资源文件（`00-设计理念.md` 第六节）。

use crate::ask::usage_line;

mod agents;
mod harness;
mod sandbox;
mod undo;

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

/// 从进程的环境里读。
pub fn current() -> Language {
    let set = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()));
    match set {
        Some(value) if value.starts_with("zh") => Language::Chinese,
        _ => Language::English,
    }
}

impl Language {
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
            Word::Skipped => ("跳过了", "skipped"),
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
            Language::Chinese => format!("没有 {name} 这个子命令。想和她对话，用 miyu ask \"…\""),
            Language::English => {
                format!("There is no {name} command. To talk to her, use miyu ask \"…\"")
            }
        }
    }

    /// 只敲了 `miyu`：终端界面还没做。
    pub fn nothing_yet(&self) -> &'static str {
        match self {
            Language::Chinese => "终端界面还没做好。想和她对话，用 miyu ask \"…\"",
            Language::English => {
                "The terminal interface is not ready yet. To talk to her, use miyu ask \"…\""
            }
        }
    }

    /// 没有可用的模型。
    pub fn no_model(&self) -> String {
        match self {
            Language::Chinese => "没有可用的模型：设环境变量 DEEPSEEK_API_KEY".to_string(),
            Language::English => "No model available: set DEEPSEEK_API_KEY".to_string(),
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
            "retryable" => ("暂时出错", "temporary error"),
            "rate_limited" => ("被限速了", "rate limited"),
            "context_too_long" => ("上下文太长", "context too long"),
            "auth" => ("认证失败", "authentication failed"),
            "content_policy" => ("被内容策略拦下了", "blocked by content policy"),
            "bad_stream" => ("回复的流不对", "bad stream"),
            "empty_reply" => ("回复是空的", "empty reply"),
            "bad_summary" => ("取不出摘要", "no summary in the reply"),
            "compaction_paused" => ("自动压缩暂停着", "automatic compaction is paused"),
            _ => ("模型出错", "model error"),
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
