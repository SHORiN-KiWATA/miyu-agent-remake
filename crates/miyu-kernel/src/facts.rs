//! 环境和状态的事实：写成什么、该不该注入（`docs/designs/08-上下文投影.md` 第五节 C10、
//! 「环境和状态的事实怎么写」）。
//!
//! 现在有两类，都由内核注入，一类一块：环境（`env`：时间、时区、工作目录）和权限级别
//! （`permission`）。每个边界查一遍：每一块和有效历史里同一个来源、同一类的最近一块比，
//! 逐字节相同就不注入（[`changed`]）。在哪些边界查，是回合状态机和压缩的事（施工 2-3、2-7、M6）。

use std::collections::BTreeMap;

use crate::event::{Body, ContextInjected, Level, Permission};
use crate::history::History;
use crate::id::FactKind;
use crate::origin::By;
use crate::template::{Template, TemplateError};
use crate::time::{Timestamp, UtcOffset};

/// 两类事实的模板：环境（`core/facts/env.txt`）和权限（`core/facts/permission.txt`）。
///
/// 由执行器从资源目录读好交进来，造会话时读一次，冻结在会话上（内核 K3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactTemplates {
    /// 环境那一块，字段是 `time`、`timezone`、`cwd`。
    env: Template,
    /// 权限那一块，字段是 `level`。
    permission: Template,
}

/// 此刻的环境，由执行器在边界上送进来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// 此刻，取自执行器的时钟。
    pub now: Timestamp,
    /// 此刻所在的时区，取自执行器。
    pub offset: UtcOffset,
    /// 工作目录，头报上来的、人看到的那种写法，例如 `~/src/miyu`。内核不改写它。
    pub cwd: String,
}

impl FactTemplates {
    /// 读两个模板。读好以后拿全部字段试着换一次：少了字段当场报错，回合里就不会再出这种错。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了这一类没有的字段，返回 [`TemplateError`]，写明哪里坏了。
    pub fn new(env: &str, permission: &str) -> Result<FactTemplates, TemplateError> {
        let templates = FactTemplates {
            env: Template::parse(env)?,
            permission: Template::parse(permission)?,
        };
        templates.env.render(&env_fields("", "", ""))?;
        templates.permission.render(&permission_fields(""))?;
        Ok(templates)
    }

    /// 环境那一块：时间到小时，时区，工作目录。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经拿全部字段试换过；`env` 这个类别名也合写法。
    pub fn env(&self, environment: &Environment) -> ContextInjected {
        let time = environment.now.local_hour(environment.offset);
        let timezone = environment.offset.to_string();
        let fields = env_fields(&time, &timezone, &environment.cwd);
        ContextInjected {
            kind: kind("env"),
            text: self.env.render(&fields).expect("造的时候试换过，字段都有"),
        }
    }

    /// 权限那一块：实际生效的那一级。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经拿全部字段试换过；`permission` 这个类别名也合写法。
    pub fn permission(&self, permission: &Permission) -> ContextInjected {
        let fields = permission_fields(effective_level(permission));
        ContextInjected {
            kind: kind("permission"),
            text: self
                .permission
                .render(&fields)
                .expect("造的时候试换过，字段都有"),
        }
    }
}

/// 这几块里该注入的，照原来的先后：和有效历史里同一个来源、同一类的最近一块比，
/// 逐字节相同的去掉（08 C10）。
///
/// 比的是最近的那一块，不是随便哪一块：先是 A，一个边界变成 B，下一个边界又回到 A，
/// 她最近看到的是 B，A 要重新注入。压缩替掉的、撤销掉的不在有效历史里，也就不算。
pub fn changed(history: &History, by: &By, facts: Vec<ContextInjected>) -> Vec<ContextInjected> {
    facts
        .into_iter()
        .filter(|fact| latest(history, by, &fact.kind) != Some(fact.text.as_str()))
        .collect()
}

/// 有效历史里，这个来源、这一类的最近一块的原文。
fn latest<'a>(history: &'a History, by: &By, kind: &FactKind) -> Option<&'a str> {
    history
        .events()
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ContextInjected(fact) if &event.by == by && &fact.kind == kind => {
                Some(fact.text.as_str())
            }
            _ => None,
        })
}

/// 实际生效的那一级：只读开着是 `read_only`，关着是常用的那一级。不认识的级别，执行时
/// 按最严的算，告诉她的也是最严的那一级。
fn effective_level(permission: &Permission) -> &'static str {
    if permission.read_only {
        return "read_only";
    }
    match permission.level {
        Level::Workspace => "workspace",
        Level::Full => "full",
        Level::Other(_) => "read_only",
    }
}

fn env_fields<'a>(time: &'a str, timezone: &'a str, cwd: &'a str) -> BTreeMap<&'a str, &'a str> {
    BTreeMap::from([("time", time), ("timezone", timezone), ("cwd", cwd)])
}

fn permission_fields(level: &str) -> BTreeMap<&str, &str> {
    BTreeMap::from([("level", level)])
}

/// 内核自己用的类别名，都合写法。
fn kind(name: &str) -> FactKind {
    FactKind::parse(name).expect("内核自己用的类别名合写法")
}

#[cfg(test)]
mod tests;
