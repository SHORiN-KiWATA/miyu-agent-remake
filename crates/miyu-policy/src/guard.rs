//! 权限策略拒绝时写给她的话（施工 4-3 下）：碰到了数据根、路径说不清在哪；只读时的写，用内核现成的那一句。
//! 和执行器替工具写的两句一样，从快照里拿。

use std::collections::BTreeMap;

use miyu_kernel::template::{Template, TemplateError};

use crate::snapshot::{BuildError, Snapshot};

/// 权限策略拒绝时写给她的三句。
#[derive(Debug, Clone)]
pub struct GuardTexts {
    forbidden: Template,
    unresolvable: Template,
    read_only: Template,
}

impl GuardTexts {
    /// `path` 在数据根里。
    pub fn forbidden(&self, path: &str) -> String {
        render(&self.forbidden, &[("path", path)])
    }

    /// `path` 换不成真实的位置，因为 `reason`。
    pub fn unresolvable(&self, path: &str, reason: &str) -> String {
        render(&self.unresolvable, &[("path", path), ("reason", reason)])
    }

    /// 只读的时候要写。
    pub fn read_only(&self) -> String {
        render(&self.read_only, &[])
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
            which: "权限策略拒绝时写的几句",
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

#[cfg(test)]
mod tests;
