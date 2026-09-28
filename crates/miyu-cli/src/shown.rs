//! 给人看的一行怎么写（施工 4-5 下写在 `ask/steps.rs` 里，施工 4-7 下挪出来，`miyu undo` 也用）：[`Line`] 分灰的、
//! 红的、绿的几段，[`Line::paint`] 照上不上色写成字；路径在工作目录里的写相对的、在家目录里的写 `~/…`；太长的截断。

use std::io::Write;
use std::path::{MAIN_SEPARATOR, Path};

/// 终端里的灰色、红色、绿色，和回到原色。
pub(crate) const GRAY: &str = "\x1b[90m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
pub(crate) const RESET: &str = "\x1b[0m";

/// 一段字是什么颜色。绿的只有差异里加上的行（施工 4-7 下）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ink {
    Gray,
    Red,
    Green,
}

/// 给人看的一行旁白，分几段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line(Vec<(Ink, String)>);

impl Line {
    /// 整行灰的。
    pub(crate) fn gray(text: impl Into<String>) -> Line {
        Line(vec![(Ink::Gray, text.into())])
    }

    /// 接着写一段。
    pub(crate) fn push(&mut self, ink: Ink, text: impl Into<String>) {
        self.0.push((ink, text.into()));
    }

    /// 写成字，带换行。`color` 的：整行包在灰色里，红的那几段换成红色，写完换回灰色，行尾回到原色，中途退出
    /// 也不会把终端留成灰的；不上色的只有字。
    pub(crate) fn paint(&self, color: bool) -> String {
        let mut out = String::new();
        let mut last = None;
        for (ink, text) in &self.0 {
            if color && last != Some(*ink) {
                out.push_str(match ink {
                    Ink::Gray => GRAY,
                    Ink::Red => RED,
                    Ink::Green => GREEN,
                });
                last = Some(*ink);
            }
            out.push_str(text);
        }
        if color {
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
