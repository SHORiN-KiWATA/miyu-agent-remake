//! 内核替工具写给模型的几句（`resources/core/tool-results/`）：英文短句，只说发生了什么
//! （`26-提示词.md` J3）。
//!
//! - 执行之前就拦下的两句，字段是 `name`，模型说的工具名（`02-内核.md` 第六节「工具怎么调、
//!   下一步怎么走」）；
//! - 打断、急着插话时补的三句，没有字段（「打断和急着插话」）；
//! - 只读时拦下的一句，没有字段（「权限级别怎么切」）；
//! - 确认的三句：被人拒绝了，没有字段；被人拒绝了、带上理由，字段是 `reason`；要人确认、这里没法
//!   确认，没有字段（「确认怎么走」）；
//! - 提问的三句：没回答，被打断了；没回答，你发了一句话；没回答，这里没有人能回答；都没有字段
//!   （「提问怎么走」）；
//! - 重启时没跑完的一句，没有字段（「载入、崩溃、重启」）。
//!
//! 字段照模板的规矩转义（`08-上下文投影.md` 第五节「模板与转义怎么写」）。原文存在策略快照里：造会话时从
//! 资源目录读进快照，执行器照快照造好交进来，冻结在会话上；载入老会话照它的快照，不再读资源目录。
//!
//! 每一句连同给人看的说法一起交回（[`Worded`]，施工 4-5 上）：说法的编号是这一份字在资源目录里的位置去掉
//! `.txt`，例如 `core/tool-results/unattended`，字段相同。

use std::collections::BTreeMap;

use crate::event::Said;
use crate::template::{Template, TemplateError};

/// 写成的一句：给模型看的字，和给人看的说法（施工 4-5 上）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worded {
    /// 给模型看的字。
    pub text: String,
    /// 给人看的说法。内核写的都有；别的模块拒绝时没交的，是空的。
    pub said: Option<Said>,
}

/// 那几句，各是一份读好的模板。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolTexts {
    /// 工具面上没有这个名字。
    unknown: Template,
    /// 参数不是一个 JSON 对象。
    not_an_object: Template,
    /// 已取消，没跑过。
    cancelled_before: Template,
    /// 已取消，跑到一半。
    cancelled_running: Template,
    /// 已跳过。
    skipped: Template,
    /// 没派：会话是只读的。
    read_only: Template,
    /// 没派：被人拒绝了。
    denied: Template,
    /// 没派：被人拒绝了，带上理由。
    denied_with_reason: Template,
    /// 没派：要人确认，这里没法确认。
    unattended: Template,
    /// 没回答：被打断了。
    question_interrupted: Template,
    /// 没回答：你发了一句话。
    question_voided: Template,
    /// 没回答：这里没有人能回答。
    question_unattended: Template,
    /// 已取消：Miyu 重启了，没跑完。
    restarted: Template,
}

/// 那几句的原文，各是一份模板。
#[derive(Debug, Clone, Copy)]
pub struct ToolTextSources<'a> {
    /// 工具面上没有这个名字，字段 `name`。
    pub unknown: &'a str,
    /// 参数不是一个 JSON 对象，字段 `name`。
    pub not_an_object: &'a str,
    /// 已取消，没跑过。
    pub cancelled_before: &'a str,
    /// 已取消，跑到一半。
    pub cancelled_running: &'a str,
    /// 已跳过。
    pub skipped: &'a str,
    /// 没派：会话是只读的。
    pub read_only: &'a str,
    /// 没派：被人拒绝了。
    pub denied: &'a str,
    /// 没派：被人拒绝了，字段 `reason`。
    pub denied_with_reason: &'a str,
    /// 没派：要人确认，这里没法确认。
    pub unattended: &'a str,
    /// 没回答：被打断了。
    pub question_interrupted: &'a str,
    /// 没回答：你发了一句话。
    pub question_voided: &'a str,
    /// 没回答：这里没有人能回答。
    pub question_unattended: &'a str,
    /// 已取消：Miyu 重启了，没跑完。
    pub restarted: &'a str,
}

impl ToolTexts {
    /// 读几份模板，读好以后拿字段试着换一次。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了不该有的字段，返回 [`TemplateError`]。
    pub fn new(sources: ToolTextSources<'_>) -> Result<ToolTexts, TemplateError> {
        let texts = ToolTexts {
            unknown: Template::parse(sources.unknown)?,
            not_an_object: Template::parse(sources.not_an_object)?,
            cancelled_before: Template::parse(sources.cancelled_before)?,
            cancelled_running: Template::parse(sources.cancelled_running)?,
            skipped: Template::parse(sources.skipped)?,
            read_only: Template::parse(sources.read_only)?,
            denied: Template::parse(sources.denied)?,
            denied_with_reason: Template::parse(sources.denied_with_reason)?,
            unattended: Template::parse(sources.unattended)?,
            question_interrupted: Template::parse(sources.question_interrupted)?,
            question_voided: Template::parse(sources.question_voided)?,
            question_unattended: Template::parse(sources.question_unattended)?,
            restarted: Template::parse(sources.restarted)?,
        };
        texts.unknown.render(&named(""))?;
        texts.not_an_object.render(&named(""))?;
        texts.denied_with_reason.render(&reasoned(""))?;
        for plain in [
            &texts.cancelled_before,
            &texts.cancelled_running,
            &texts.skipped,
            &texts.read_only,
            &texts.denied,
            &texts.unattended,
            &texts.question_interrupted,
            &texts.question_voided,
            &texts.question_unattended,
            &texts.restarted,
        ] {
            plain.render(&BTreeMap::new())?;
        }
        Ok(texts)
    }

    /// 工具面上没有叫 `name` 的工具。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn unknown(&self, name: &str) -> Worded {
        word(&self.unknown, "unknown", &named(name))
    }

    /// 给 `name` 的参数不是一个 JSON 对象。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn not_an_object(&self, name: &str) -> Worded {
        word(&self.not_an_object, "not-an-object", &named(name))
    }

    /// 已取消，没跑过。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn cancelled_before(&self) -> Worded {
        word(&self.cancelled_before, "cancelled-before", &BTreeMap::new())
    }

    /// 已取消，跑到一半。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn cancelled_running(&self) -> Worded {
        word(
            &self.cancelled_running,
            "cancelled-running",
            &BTreeMap::new(),
        )
    }

    /// 已跳过。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn skipped(&self) -> Worded {
        word(&self.skipped, "skipped", &BTreeMap::new())
    }

    /// 没派：会话是只读的。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn read_only(&self) -> Worded {
        word(&self.read_only, "read-only", &BTreeMap::new())
    }

    /// 没派：被人拒绝了。写了理由的，带上 `reason`。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn denied(&self, reason: Option<&str>) -> Worded {
        match reason {
            Some(reason) => word(
                &self.denied_with_reason,
                "denied-with-reason",
                &reasoned(reason),
            ),
            None => word(&self.denied, "denied", &BTreeMap::new()),
        }
    }

    /// 没派：要人确认，这里没法确认。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn unattended(&self) -> Worded {
        word(&self.unattended, "unattended", &BTreeMap::new())
    }

    /// 没回答：被打断了。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn question_interrupted(&self) -> Worded {
        word(
            &self.question_interrupted,
            "question-interrupted",
            &BTreeMap::new(),
        )
    }

    /// 没回答：你发了一句话。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn question_voided(&self) -> Worded {
        word(&self.question_voided, "question-voided", &BTreeMap::new())
    }

    /// 没回答：这里没有人能回答。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn question_unattended(&self) -> Worded {
        word(
            &self.question_unattended,
            "question-unattended",
            &BTreeMap::new(),
        )
    }

    /// 已取消：Miyu 重启了，没跑完。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn restarted(&self) -> Worded {
        word(&self.restarted, "restarted", &BTreeMap::new())
    }
}

fn render(template: &Template, fields: &BTreeMap<&str, &str>) -> String {
    template.render(fields).expect("造的时候试换过，字段都有")
}

/// 照 `template` 写成一句，说法是 `core/tool-results/<name>`，字段相同。
fn word(template: &Template, name: &str, fields: &BTreeMap<&str, &str>) -> Worded {
    let mut said = Said::new(format!("core/tool-results/{name}"));
    for (field, value) in fields {
        said = said.with(field, *value);
    }
    Worded {
        text: render(template, fields),
        said: Some(said),
    }
}

fn named(name: &str) -> BTreeMap<&str, &str> {
    BTreeMap::from([("name", name)])
}

fn reasoned(reason: &str) -> BTreeMap<&str, &str> {
    BTreeMap::from([("reason", reason)])
}

#[cfg(test)]
mod tests;
