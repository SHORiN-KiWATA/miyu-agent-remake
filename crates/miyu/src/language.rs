//! 给人看的话跟着界面语言（`docs/designs/22-命令行.md` 第二节）：照 `LC_ALL`、`LC_MESSAGES`、`LANG`，`zh`
//! 开头的说中文，别的说英文。配置系统做出来以后照这个人的界面语言设置；界面的字先写在这里，做界面
//! 语言的那一步挪进资源文件（`00-设计理念.md` 第六节）。

/// 界面语言。
pub(crate) enum Language {
    /// 中文。
    Chinese,
    /// 英文。
    English,
}

/// 从进程的环境里读。
pub(crate) fn current() -> Language {
    let set = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()));
    match set {
        Some(value) if value.starts_with("zh") => Language::Chinese,
        _ => Language::English,
    }
}

impl Language {
    /// 没有这个子命令。
    pub(crate) fn no_such_command(&self, name: &str) -> String {
        match self {
            Language::Chinese => format!("没有 {name} 这个子命令。想和她对话，用 miyu ask \"…\""),
            Language::English => {
                format!("There is no {name} command. To talk to her, use miyu ask \"…\"")
            }
        }
    }

    /// 只敲了 `miyu`：终端界面还没做。
    pub(crate) fn nothing_yet(&self) -> &'static str {
        match self {
            Language::Chinese => "终端界面还没做好。想和她对话，用 miyu ask \"…\"",
            Language::English => {
                "The terminal interface is not ready yet. To talk to her, use miyu ask \"…\""
            }
        }
    }
}
