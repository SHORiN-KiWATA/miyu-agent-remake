//! 了结以后正文里留什么（蓝图「确认和提问的抽屉」第 7 条），和抽屉、「确认」页共用的几样写法：
//! 一道的回答写成一句、确认的问题行、碰到的路径。

use super::{Answer, Ask, Decision, Drawer, Outcome, Texts};
use crate::local::home_short;

/// 了结以后留什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// 提问答了：一道一行「问题：回答」，接在时间线里「提问」那一步下面（第 6 条，2026-10-11 项目主人）。
    Answers(Vec<String>),
    /// 不允许：红 `✗` 一行。
    Denied(String),
    /// 什么都不留：允许了、取消了（取消一定顺带打断这一轮，那一步的结果和「已中断」已经说了，2026-10-11 项目主人）。
    Nothing,
}

impl Drawer {
    /// 了结以后留什么。
    pub fn report(&self, outcome: &Outcome, texts: &Texts) -> Report {
        match outcome {
            Outcome::Answered(a) => {
                let lines = a.answers.iter().enumerate().map(|(p, answer)| {
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
                });
                Report::Answers(lines.collect())
            }
            Outcome::Cancelled => Report::Nothing,
            Outcome::Decided(d) if d.decision != Decision::Deny => Report::Nothing,
            Outcome::Decided(d) => {
                let text = d.reason.as_ref().map_or_else(
                    || texts.decisions[2].clone(),
                    |r| texts.denied_with.replace("{reason}", &inline(r)),
                );
                Report::Denied(text)
            }
        }
    }

    /// 确认的问题行：「要写 1 个文件」这类，照 `access` 找（第 3 条）。
    pub fn title(&self, texts: &Texts) -> String {
        let Ask::Approval(a) = &self.ask else {
            return String::new();
        };
        // 跑命令的：她写的短标题当问题行（2026-10-07 项目主人：不写「要用 shell」）。
        if let Some((Some(title), _)) = &self.command {
            return title.clone();
        }
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
