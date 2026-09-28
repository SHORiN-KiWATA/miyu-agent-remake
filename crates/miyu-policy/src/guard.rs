//! 权限策略拒绝时写给她的话（施工 4-3 下）：碰到了数据根、路径说不清在哪；只读时的写，用内核现成的那一句。
//! 和执行器替工具写的两句一样，从快照里拿。

use std::collections::BTreeMap;

use miyu_kernel::event::Said;
use miyu_kernel::template::{Template, TemplateError};
use miyu_kernel::tool::Worded;

use crate::snapshot::{BuildError, Snapshot};

/// 权限策略拒绝时写给她的三句。
#[derive(Debug, Clone)]
pub struct GuardTexts {
    forbidden: Template,
    unresolvable: Template,
    read_only: Template,
}

impl GuardTexts {
    /// `path` 在数据根里。说法是 `core/permissions/forbidden`（施工 4-5 上）。
    pub fn forbidden(&self, path: &str) -> Worded {
        word(
            &self.forbidden,
            "core/permissions/forbidden",
            &[("path", path)],
        )
    }

    /// `path` 换不成真实的位置，因为 `reason`。说法是 `core/permissions/unresolvable`。
    pub fn unresolvable(&self, path: &str, reason: &str) -> Worded {
        word(
            &self.unresolvable,
            "core/permissions/unresolvable",
            &[("path", path), ("reason", reason)],
        )
    }

    /// 只读的时候要写。和内核现成的那一句一样，说法是 `core/tool-results/read-only`。
    pub fn read_only(&self) -> Worded {
        word(&self.read_only, "core/tool-results/read-only", &[])
    }
}

impl Snapshot {
    /// 权限策略拒绝时写给她的三句。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了不该有的字段。
    pub fn guard_texts(&self) -> Result<GuardTexts, BuildError> {
        let texts = &self.core.permissions;
        let read_only = &self.core.tool_results.read_only;
        let build = || -> Result<GuardTexts, TemplateError> {
            Ok(GuardTexts {
                forbidden: parse(&texts.forbidden, &[("path", "")])?,
                unresolvable: parse(&texts.unresolvable, &[("path", ""), ("reason", "")])?,
                read_only: parse(read_only, &[])?,
            })
        };
        build().map_err(|error| BuildError::Texts {
            which: "permission denial texts",
            error,
        })
    }
}

/// 读一份模板，拿空的字段试换一次。
fn parse(source: &str, fields: &[(&str, &str)]) -> Result<Template, TemplateError> {
    let template = Template::parse(source)?;
    template.render(&fields.iter().copied().collect::<BTreeMap<_, _>>())?;
    Ok(template)
}

/// 换进字段。
///
/// # Panics
///
/// 实际不会 panic：造的时候已经试换过。
fn render(template: &Template, fields: &[(&str, &str)]) -> String {
    template
        .render(&fields.iter().copied().collect::<BTreeMap<_, _>>())
        .expect("造的时候试换过，字段都有")
}

/// 写成一句，连同给人看的说法 `key`：字段相同。
fn word(template: &Template, key: &str, fields: &[(&str, &str)]) -> Worded {
    let said = fields.iter().fold(Said::new(key), |said, (field, value)| {
        said.with(field, *value)
    });
    Worded {
        text: render(template, fields),
        said: Some(said),
    }
}

#[cfg(test)]
mod tests;
