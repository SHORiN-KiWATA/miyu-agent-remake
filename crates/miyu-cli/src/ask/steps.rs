//! 她做的每一步（`docs/designs/22-命令行.md` 第三节「施工 4-5 下定的」）：记下她调了什么，结果来了写成给人
//! 看的一行，例如 `· 读取 src/lib.rs → 37 行`；没做成的同一行写原因，「出错」「没做」是红的。工作目录太宽、
//! 核心退回账号的工作区时开头那一句也在这里写。
//!
//! 只管写成什么样，不管往哪写：[`Line`] 分灰的、红的几段，[`Line::paint`] 照上不上色写成字。

use std::collections::BTreeMap;
use std::path::{MAIN_SEPARATOR, Path};

use serde_json::Value;

use miyu_kernel::event::Said;
use miyu_store::human::clean;

use super::Plan;
use crate::language::Word;

/// 终端里的灰色、红色，和回到原色。
pub(crate) const GRAY: &str = "\x1b[90m";
const RED: &str = "\x1b[31m";
pub(crate) const RESET: &str = "\x1b[0m";

/// 参数的值最多印几个字。
const SUBJECT_CHARS: usize = 80;
/// 结果那一句最多印几个字。
const RESULT_CHARS: usize = 120;
/// 没有显示名的工具，工具名最多印几个字。
const NAME_CHARS: usize = 40;

/// 参数叫这两个名字的是路径（`10-自带软件.md` 第十节定的名字）。
const PATHS: [&str; 2] = ["file_path", "path"];

/// 一段字是什么颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ink {
    Gray,
    Red,
}

/// 给人看的一行旁白，分几段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line(Vec<(Ink, String)>);

impl Line {
    /// 整行灰的。
    fn gray(text: impl Into<String>) -> Line {
        Line(vec![(Ink::Gray, text.into())])
    }

    /// 接着写一段。
    fn push(&mut self, ink: Ink, text: impl Into<String>) {
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

/// 她调过的，照调用编号记着：好在结果来了时知道是哪件工具、给了什么参数。
#[derive(Debug, Default)]
pub(crate) struct Steps {
    calls: BTreeMap<String, Called>,
}

/// 调过的一次。
#[derive(Debug)]
struct Called {
    /// 她写的工具名。
    name: String,
    /// 参数：读不懂的是空的。
    args: Value,
}

impl Steps {
    /// 一次回复（`message.assistant` 的 `body`）：记下里面的工具调用。
    pub(crate) fn reply(&mut self, body: &Value) {
        for block in body["blocks"].as_array().into_iter().flatten() {
            if block["type"] != "tool_call" {
                continue;
            }
            let (Some(id), Some(name)) = (block["call_id"].as_str(), block["name"].as_str()) else {
                continue;
            };
            let args = block["args"]
                .as_str()
                .and_then(|args| serde_json::from_str(args).ok())
                .unwrap_or(Value::Null);
            let name = name.to_string();
            self.calls.insert(id.to_string(), Called { name, args });
        }
    }

    /// 一次结果（`tool.result` 的 `body`）写成的那一行。路径照会话实际干活的目录 `cwd` 写短。对不上她调过的
    /// 哪一次的没有：掉队重订以后，前面的推送没看到，不猜。
    pub(crate) fn result(&self, body: &Value, plan: &Plan, cwd: &str) -> Option<Line> {
        let called = self.calls.get(body["call_id"].as_str()?)?;
        let face = plan.human.tool(&called.name);
        let mut head = match face {
            Some(face) => format!("· {}", face.name),
            None => format!("· {}", cut(&clean(&called.name), NAME_CHARS)),
        };
        let subject = face.and_then(|face| face.subject.as_deref());
        if let Some(value) =
            subject.and_then(|subject| value_of(&called.args, subject, cwd, plan.home.as_deref()))
        {
            head.push(' ');
            head.push_str(&value);
        }
        let said = body
            .get("human")
            .and_then(|human| serde_json::from_value::<Said>(human.clone()).ok())
            .and_then(|said| plan.human.say(&said))
            .map(|text| cut(&text, RESULT_CHARS));
        let language = &plan.language;
        let status = body["status"].as_str().unwrap_or_default();
        let failed = match status {
            "error" => Some(Word::Failed),
            "denied" => Some(Word::Denied),
            _ => None,
        };
        let plain = match status {
            "cancelled" => Some(Word::Cancelled),
            "skipped" => Some(Word::Skipped),
            _ => None,
        };
        let mut line = Line::gray(head);
        match (failed, said) {
            // 没做成的：红的那个词，有原因的跟上原因。
            (Some(word), said) => {
                line.push(Ink::Gray, " → ");
                line.push(Ink::Red, language.word(word));
                if let Some(said) = said {
                    line.push(Ink::Gray, format!("{}{said}", language.colon()));
                }
            }
            (None, Some(said)) => line.push(Ink::Gray, format!(" → {said}")),
            // 没有说法的：打断了、跳过了照状态写；做成了的只写做了什么。
            (None, None) => {
                if let Some(word) = plain {
                    line.push(Ink::Gray, format!(" → {}", language.word(word)));
                }
            }
        }
        Some(line)
    }
}

/// 工作目录太宽、核心退回了账号的工作区时的那一句：敲命令时在 `plan.cwd`，实际在 `used` 里干活。
pub(crate) fn moved(plan: &Plan, used: &str) -> Line {
    let home = plan.home.as_deref();
    let (given, used) = (clean(&tilde(&plan.cwd, home)), clean(&tilde(used, home)));
    Line::gray(plan.language.moved(&given, &used))
}

/// 参数 `subject` 的值，写成给人看的：只取第一行，有第二行的加 `…`；路径写短；控制字符换掉；太长的截断，
/// 路径留后面（文件名在后面），别的留前面。没有这个参数、不是字符串、是空的，都没有。
fn value_of(args: &Value, subject: &str, cwd: &str, home: Option<&Path>) -> Option<String> {
    let value = args.get(subject)?.as_str()?;
    let mut lines = value.lines();
    let first = lines.next()?;
    let more = lines.next().is_some();
    if PATHS.contains(&subject) {
        let path = cut_front(&clean(&shown(first, cwd, home)), SUBJECT_CHARS);
        return Some(if more { format!("{path}…") } else { path });
    }
    let mut text = cut(&clean(first), SUBJECT_CHARS);
    if more && !text.ends_with('…') {
        text.push('…');
    }
    Some(text)
}

/// 路径写成给人看的：在工作目录 `cwd` 里的写相对的（工作目录本身写 `.`），在家目录里的写 `~/…`，别的照原样
/// （`10-自带软件.md` 第十节：工具结果里的路径也这样写）。
fn shown(path: &str, cwd: &str, home: Option<&Path>) -> String {
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
fn tilde(path: &str, home: Option<&Path>) -> String {
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
fn cut(text: &str, most: usize) -> String {
    match text.char_indices().nth(most) {
        None => text.to_string(),
        Some((at, _)) => format!("{}…", &text[..at]),
    }
}

/// 超过 `most` 个字的，只留后面 `most` 个，前面加 `…`。
fn cut_front(text: &str, most: usize) -> String {
    let count = text.chars().count();
    if count <= most {
        return text.to_string();
    }
    match text.char_indices().nth(count - most) {
        Some((at, _)) => format!("…{}", &text[at..]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests;
