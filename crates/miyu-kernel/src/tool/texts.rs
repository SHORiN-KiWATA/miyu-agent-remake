//! 内核替工具写给模型的几句（`resources/core/tool-results/`）：英文短句，只说发生了什么
//! （`26-提示词.md` J3）。
//!
//! - 执行之前就拦下的两句，字段是 `name`，模型说的工具名（`02-内核.md` 第六节「工具怎么调、
//!   下一步怎么走」）；
//! - 打断、急着插话时补的三句，没有字段（「打断和急着插话」）；
//! - 只读时拦下的一句，没有字段（「权限级别怎么切」）；
//! - 确认的三句：被人拒绝了，没有字段；被人拒绝了、带上理由，字段是 `reason`；要人确认、这里没法
//!   确认，没有字段（「确认怎么走」）。
//!
//! 字段照模板的规矩转义（`08-上下文投影.md` 第五节「模板与转义怎么写」）。由执行器从资源目录读好
//! 交进来，造会话时读一次，冻结在会话上。

use std::collections::BTreeMap;

use crate::template::{Template, TemplateError};

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
    pub fn unknown(&self, name: &str) -> String {
        render(&self.unknown, &named(name))
    }

    /// 给 `name` 的参数不是一个 JSON 对象。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn not_an_object(&self, name: &str) -> String {
        render(&self.not_an_object, &named(name))
    }

    /// 已取消，没跑过。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn cancelled_before(&self) -> String {
        render(&self.cancelled_before, &BTreeMap::new())
    }

    /// 已取消，跑到一半。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn cancelled_running(&self) -> String {
        render(&self.cancelled_running, &BTreeMap::new())
    }

    /// 已跳过。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn skipped(&self) -> String {
        render(&self.skipped, &BTreeMap::new())
    }

    /// 没派：会话是只读的。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn read_only(&self) -> String {
        render(&self.read_only, &BTreeMap::new())
    }

    /// 没派：被人拒绝了。写了理由的，带上 `reason`。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn denied(&self, reason: Option<&str>) -> String {
        match reason {
            Some(reason) => render(&self.denied_with_reason, &reasoned(reason)),
            None => render(&self.denied, &BTreeMap::new()),
        }
    }

    /// 没派：要人确认，这里没法确认。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn unattended(&self) -> String {
        render(&self.unattended, &BTreeMap::new())
    }
}

fn render(template: &Template, fields: &BTreeMap<&str, &str>) -> String {
    template.render(fields).expect("造的时候试换过，字段都有")
}

fn named(name: &str) -> BTreeMap<&str, &str> {
    BTreeMap::from([("name", name)])
}

fn reasoned(reason: &str) -> BTreeMap<&str, &str> {
    BTreeMap::from([("reason", reason)])
}

#[cfg(test)]
mod tests;
