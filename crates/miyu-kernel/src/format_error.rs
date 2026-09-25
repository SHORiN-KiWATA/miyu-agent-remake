//! 读到的文字不合图纸上的写法（`docs/designs/03-事件模型.md` 第二节）。

use std::fmt;

/// 一段文字不合图纸上的写法：读的是什么、错在哪、读到了什么。
///
/// 编号、名字、时间读不进来，都用它报错。报错是中文，给查问题的人看；
/// 要报给模型的错另写英文（`26-提示词.md` J3），不要把它原样转给模型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    /// 读的是什么，例如「会话编号」。
    pub what: &'static str,
    /// 读到的原文，太长的只留开头。
    pub text: String,
    /// 错在哪。
    pub why: &'static str,
}

impl FormatError {
    pub(crate) fn new(what: &'static str, text: &str, why: &'static str) -> Self {
        const KEEP: usize = 80;
        let text = match text.char_indices().nth(KEEP) {
            Some((cut, _)) => format!("{}…", &text[..cut]),
            None => text.to_string(),
        };
        Self { what, text, why }
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}的写法不对：{}（读到的是 {:?}）",
            self.what, self.why, self.text
        )
    }
}

impl std::error::Error for FormatError {}
