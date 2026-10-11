//! 给人看的一行怎么写（施工 4-5 下写在 `ask/steps.rs` 里，施工 4-7 下挪出来，`miyu undo` 也用）：[`Line`] 分原色的、
//! 灰的、红的、绿的几段，[`Line::paint`] 照上不上色写成字；路径在工作目录里的写相对的、在家目录里的写 `~/…`；太长的
//! 截断；别人给的字里的控制字符怎么处理。上不上色照 [`colored`]。

use std::ffi::OsStr;
use std::io::Write;
use std::path::{MAIN_SEPARATOR, Path};

use miyu_kernel::time::UtcOffset;

/// 终端里的灰色、红色、绿色，和回到原色。
pub(crate) const GRAY: &str = "\x1b[90m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
pub(crate) const RESET: &str = "\x1b[0m";

/// 上不上色：写到的是终端，`NO_COLOR` 又没设或者设成空的。no-color.org 的约定是设了、不是空的才不上色（施工 4-9
/// 再补四上：原来设成空的也不上色）。
pub(crate) fn colored(terminal: bool, no_color: Option<&OsStr>) -> bool {
    terminal && no_color.is_none_or(OsStr::is_empty)
}

/// 一段字是什么颜色。绿的只有差异里加上的行（施工 4-7 下）；原色的是每一步的标题、执行命令的输出（施工 4-11）；黄的
/// 只有配置的警告（施工 8-2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ink {
    Plain,
    Gray,
    Red,
    Green,
    /// 警告（施工 8-2，`miyu check`）。
    Yellow,
}

impl Ink {
    /// 换成这个颜色要写的：原色写 `ESC[0m`。
    fn code(self) -> &'static str {
        match self {
            Ink::Plain => RESET,
            Ink::Gray => GRAY,
            Ink::Red => RED,
            Ink::Green => GREEN,
            Ink::Yellow => YELLOW,
        }
    }
}

/// 给人看的一行旁白，分几段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line(Vec<(Ink, String)>);

impl Line {
    /// 整行灰的。
    pub(crate) fn gray(text: impl Into<String>) -> Line {
        Line(vec![(Ink::Gray, text.into())])
    }

    /// 整行一种颜色。
    pub(crate) fn inked(ink: Ink, text: impl Into<String>) -> Line {
        Line(vec![(ink, text.into())])
    }

    /// 接着写一段。
    pub(crate) fn push(&mut self, ink: Ink, text: impl Into<String>) {
        self.0.push((ink, text.into()));
    }

    /// 写成字，带换行。`color` 的：换颜色时写新颜色，换回原色写 `ESC[0m`；上过色的行，行尾再写一个 `ESC[0m`，
    /// 中途退出也不会把终端留成灰的。整行原色的、不上色的，只有字。
    pub(crate) fn paint(&self, color: bool) -> String {
        let mut out = String::new();
        let mut last = Ink::Plain;
        let mut painted = false;
        for (ink, text) in &self.0 {
            if color && *ink != last {
                out.push_str(ink.code());
                last = *ink;
                painted = true;
            }
            out.push_str(text);
        }
        if painted {
            out.push_str(RESET);
        }
        out.push('\n');
        out
    }
}

/// 路径写成给人看的：在工作目录 `cwd` 里的写相对的（工作目录本身写 `.`），在家目录里的写 `~/…`，别的照原样
/// （`10-自带软件.md` 第十节：工具结果里的路径也这样写）。
pub(crate) fn shown(path: &str, cwd: &str, home: Option<&Path>) -> String {
    let full = Path::new(path);
    if full.is_absolute()
        && let Ok(rest) = full.strip_prefix(cwd)
    {
        return match rest.as_os_str().is_empty() {
            true => ".".to_string(),
            false => rest.display().to_string(),
        };
    }
    tilde(path, home)
}

/// 在家目录里的路径写成 `~/…`，家目录本身写 `~`；别的照原样。
pub(crate) fn tilde(path: &str, home: Option<&Path>) -> String {
    let full = Path::new(path);
    if let Some(home) = home
        && full.is_absolute()
        && let Ok(rest) = full.strip_prefix(home)
    {
        return match rest.as_os_str().is_empty() {
            true => "~".to_string(),
            false => format!("~{MAIN_SEPARATOR}{}", rest.display()),
        };
    }
    path.to_string()
}

/// 别人给的一行字（她写的参数、命令的输出）写成给人看的：控制字符换成 `�`，制表符照原样留着（用制表符缩进的
/// 文件、输出，才对得齐）。
pub(crate) fn keep_tabs(row: &str) -> String {
    row.chars().map(defused).collect()
}

/// 命令输出里的一行写成给人看的（施工 4-11）：终端的控制序列里，CSI（改颜色、挪光标）、OSC（改标题、写剪贴板）两种
/// 整段去掉；别的控制字符换成 `�`，制表符照留。CSI 写坏了的，只去掉认得出的那一截；OSC 到行尾都没收尾的，去到行尾。
pub(crate) fn strip_escapes(row: &str) -> String {
    let mut out = String::with_capacity(row.len());
    let mut chars = row.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(defused(c));
            continue;
        }
        if chars.next_if_eq(&'[').is_some() {
            while chars
                .next_if(|c| ('\u{20}'..='\u{3f}').contains(c))
                .is_some()
            {}
            chars.next_if(|c| ('\u{40}'..='\u{7e}').contains(c));
        } else if chars.next_if_eq(&']').is_some() {
            while let Some(c) = chars.next() {
                if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                    break;
                }
            }
        } else {
            out.push('\u{FFFD}');
        }
    }
    out
}

/// 一个字：控制字符换成 `�`，制表符照留。
fn defused(c: char) -> char {
    if c.is_control() && c != '\t' {
        '\u{FFFD}'
    } else {
        c
    }
}

/// 超过 `most` 个字的，截到 `most` 个，末尾加 `…`。
pub(crate) fn cut(text: &str, most: usize) -> String {
    match text.char_indices().nth(most) {
        None => text.to_string(),
        Some((at, _)) => format!("{}…", &text[..at]),
    }
}

/// 超过 `most` 个字的，只留后面 `most` 个，前面加 `…`。
pub(crate) fn cut_front(text: &str, most: usize) -> String {
    let count = text.chars().count();
    if count <= most {
        return text.to_string();
    }
    match text.char_indices().nth(count - most) {
        Some((at, _)) => format!("…{}", &text[at..]),
        None => text.to_string(),
    }
}

/// 写一段，马上送出去：边收边打。写不出去的不管（例如标准错误被关了），不影响别的。
#[expect(clippy::let_underscore_must_use, reason = "写不出去也没有别处可说")]
pub(crate) fn write(to: &mut dyn Write, text: &str) {
    let _ = to.write_all(text.as_bytes()).and_then(|()| to.flush());
}

/// 说一句话，带换行。
pub(crate) fn say(to: &mut dyn Write, line: &str) {
    write(to, &format!("{line}\n"));
}

/// 这台机器此刻的时区（照核心给会话的环境的算法，`miyu-endpoint` 的 `sessions.rs`）：`miyu memory` 的日期、`miyu pkg info`
/// 的安装时间照它换。
pub(crate) fn offset() -> UtcOffset {
    let minutes = jiff::Zoned::now().offset().seconds() / 60;
    UtcOffset::from_minutes(minutes).unwrap_or(UtcOffset::UTC)
}

#[cfg(test)]
mod tests;
