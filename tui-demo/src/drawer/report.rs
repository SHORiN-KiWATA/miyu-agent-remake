//! 了结以后正文里留什么（蓝图「确认和提问的抽屉」第 7 条），和抽屉、「确认」页共用的几样写法：
//! 一道的回答写成一句、确认的问题行、碰到的路径。

use super::{Answer, Ask, Decision, Drawer, Outcome, Texts};
use crate::local::home_short;

/// 一行结果的记号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// 不允许：红 `✗`。
    Bad,
    /// 取消了：没有记号，整行暗。
    Void,
}

/// 了结以后留什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// 提问答了：旧版的引用块，一行一个字符串。
    Block(Vec<String>),
    /// 一行：不允许、取消。
    Line(Mark, String),
    /// 什么都不留：允许了。
    Nothing,
}

impl Drawer {
    /// 了结以后留什么。
    pub fn report(&self, outcome: &Outcome, texts: &Texts) -> Report {
        match outcome {
            Outcome::Answered(a) => {
                let mut lines = vec![texts.answered_title.clone()];
                lines.extend(a.answers.iter().enumerate().map(|(p, answer)| {
                    let said = if empty(answer) {
                        texts.unanswered.clone()
                    } else {
                        said(answer)
                    };
                    let mut line = texts
                        .answered_line
                        .replace("{label}", &self.label(p))
                        .replace("{answer}", &said);
                    if let Some(notes) = &answer.notes {
                        line.push_str(&texts.notes_suffix.replace("{notes}", &inline(notes)));
                    }
                    line
                }));
                Report::Block(lines)
            }
            Outcome::Cancelled => Report::Line(Mark::Void, texts.cancelled.clone()),
            Outcome::Decided(d) if d.decision != Decision::Deny => Report::Nothing,
            Outcome::Decided(d) => {
                let text = d.reason.as_ref().map_or_else(
                    || texts.decisions[3].clone(),
                    |r| texts.denied_with.replace("{reason}", &inline(r)),
                );
                Report::Line(Mark::Bad, text)
            }
        }
    }

    /// 确认的问题行：「要写 1 个文件」这类，照 `access` 找（第 3 条）。
    pub fn title(&self, texts: &Texts) -> String {
        let Ask::Approval(a) = &self.ask else {
            return String::new();
        };
        let detail = a.detail.clone().unwrap_or_default();
        let template = texts.access.get(&a.access).unwrap_or(&texts.access_other);
        template
            .replace("{count}", &detail.paths.len().to_string())
            .replace("{tool}", detail.tool.as_deref().unwrap_or(&a.access))
    }

    /// 确认碰到的路径：家目录写 `~`，工作区外的带上标记。
    pub fn paths(&self, texts: &Texts) -> Vec<(String, Option<String>)> {
        let Ask::Approval(a) = &self.ask else {
            return Vec::new();
        };
        a.detail
            .iter()
            .flat_map(|d| &d.paths)
            .map(|p| {
                let outside = p.zone.as_deref() == Some("outside");
                (home_short(&p.path), outside.then(|| texts.outside.clone()))
            })
            .collect()
    }
}

/// 什么都没答（补充不算）。
fn empty(answer: &Answer) -> bool {
    answer.picked.is_empty() && answer.text.is_none()
}

/// 一道的回答写成一句：选了的用「、」接起来，再接自己写的话。
pub fn said(answer: &Answer) -> String {
    let mut parts = answer.picked.clone();
    parts.extend(answer.text.as_deref().map(inline));
    parts.join("、")
}

/// 写成一行：换行写成 `↵`（照旧版）。
pub fn inline(text: &str) -> String {
    text.trim().replace(['\r', '\n'], "↵")
}
