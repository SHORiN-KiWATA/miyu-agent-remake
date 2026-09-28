//! 她做的每一步（`docs/designs/22-命令行.md` 第三节「施工 4-5 下定的」）：记下她调了什么，结果来了写成给人
//! 看的一行，例如 `· 读取 src/lib.rs → 37 行`；没做成的同一行写原因，「出错」「没做」是红的。工作目录太宽、
//! 核心退回账号的工作区时开头那一句，有几步因为要确认没做时最后那一句（施工 4-9），也在这里写。
//!
//! 只管写成什么样，不管往哪写：一行分灰的、红的几段（[`Line`]，在 `shown.rs` 里）。

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use miyu_kernel::event::Said;
use miyu_store::human::clean;

use super::Plan;
use crate::language::Word;
use crate::shown::{Ink, Line, cut, cut_front, shown, tilde};

/// 参数的值最多印几个字。
const SUBJECT_CHARS: usize = 80;
/// 结果那一句最多印几个字。
const RESULT_CHARS: usize = 120;
/// 没有显示名的工具，工具名最多印几个字。
const NAME_CHARS: usize = 40;

/// 参数叫这两个名字的是路径（`10-自带软件.md` 第十节定的名字）。
const PATHS: [&str; 2] = ["file_path", "path"];

/// 内核在没人能确认时记的那一句（`02-内核.md` 第六节「确认怎么走」第 2 条）。
const UNATTENDED: &str = "core/tool-results/unattended";

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

/// 一次结果（`tool.result` 的 `body`）是不是因为要确认、这里没人能确认被拒的：状态是 `denied`，说法是内核的那一句。
/// 别的拒绝不算：只读时要写的、碰到数据根的，都不是要确认（施工 4-9）。
pub(crate) fn unattended(body: &Value) -> bool {
    body["status"] == "denied" && body["human"]["key"] == UNATTENDED
}

/// 有 `steps` 步因为要确认没做时，最后印的那一行：`· 1 步没做：要你确认，miyu ask 里确认不了`，「没做」是红的
/// （`22-命令行.md` O3，施工 4-9）。
pub(crate) fn unattended_line(plan: &Plan, steps: u64) -> Line {
    let language = &plan.language;
    let (before, after) = language.unattended(steps);
    let mut line = Line::gray(before);
    line.push(Ink::Red, language.word(Word::Denied));
    line.push(Ink::Gray, after);
    line
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

#[cfg(test)]
mod tests;
