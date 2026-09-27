//! 给人看的话跟着界面语言（`docs/designs/22-命令行.md` 第二节）：照 `LC_ALL`、`LC_MESSAGES`、`LANG`，`zh`
//! 开头的说中文，别的说英文。配置系统做出来以后照这个人的界面语言设置；界面的字先写在这里，做界面
//! 语言的那一步挪进资源文件（`00-设计理念.md` 第六节）。

use crate::ask::usage_line;

/// 界面语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// 中文。
    Chinese,
    /// 英文。
    English,
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
    fn class(&self, class: &str) -> &'static str {
        let (chinese, english) = match class {
            "retryable" => ("暂时出错", "temporary error"),
            "rate_limited" => ("被限速了", "rate limited"),
            "context_too_long" => ("上下文太长", "context too long"),
            "auth" => ("认证失败", "authentication failed"),
            "content_policy" => ("被内容策略拦下了", "blocked by content policy"),
            "bad_stream" => ("回复的流不对", "bad stream"),
            "empty_reply" => ("回复是空的", "empty reply"),
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

    /// 核心拒绝了：它的原话已经照握手时报的语言说了。
    pub fn refused(&self, reason: &str) -> String {
        reason.to_string()
    }

    /// 问完那一行用量。
    pub(crate) fn usage(&self, sum: &crate::ask::Sum) -> String {
        usage_line(self, sum)
    }

    /// `miyu ask` 是做什么的。
    pub fn ask_about(&self) -> &'static str {
        match self {
            Language::Chinese => "说一句话，打印她的回答",
            Language::English => "Say something and print her answer",
        }
    }

    /// 要说的话。
    pub fn words_help(&self) -> &'static str {
        match self {
            Language::Chinese => "要说的话",
            Language::English => "What to say",
        }
    }

    /// `--session`。
    pub fn session_help(&self) -> &'static str {
        match self {
            Language::Chinese => "接着这个会话说",
            Language::English => "Go on in this session",
        }
    }

    /// `--continue`。
    pub fn continue_help(&self) -> &'static str {
        match self {
            Language::Chinese => "接着上一次 miyu ask 开的会话说",
            Language::English => "Go on in the session the last miyu ask opened",
        }
    }

    /// `--format`。
    pub fn format_help(&self) -> &'static str {
        match self {
            Language::Chinese => "输出的格式：text 给人看，json 给脚本",
            Language::English => "Output format: text for people, json for scripts",
        }
    }
}
