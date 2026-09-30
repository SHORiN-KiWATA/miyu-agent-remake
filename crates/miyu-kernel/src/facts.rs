//! 环境和状态的事实：写成什么、该不该注入（`docs/designs/08-上下文投影.md` 第五节 C10、
//! 「环境和状态的事实怎么写」）。
//!
//! 现在有三类，都由内核注入，一类一块：环境（`env`：时间、时区、工作目录）、权限级别
//! （`permission`）和会话编号（`session`，施工 1-13 再补）。每个边界查一遍：每一块和有效历史里同一个来源、
//! 同一类的最近一块比，逐字节相同就不注入（[`changed`]）。在哪些边界查，是回合状态机和压缩的事（施工 2-3、2-7、M6）。
//! 会话编号一个会话里不变，所以只在第一轮、压缩以后、撤掉带着它的那一轮以后注入；它不并进环境那一块：环境每过整点
//! 重发，编号跟着重发就白花。
//!
//! 还有一块不是环境和状态，是发生了的事：回复被出错打断了（`reply_cut`，施工 3-5 下）。它跟在
//! 半截回复后面，每次都注入，不和以前的比。

use std::collections::BTreeMap;

use crate::event::{Body, ContextInjected, Level, Permission};
use crate::history::History;
use crate::id::{FactKind, SessionId};
use crate::origin::By;
use crate::template::{Template, TemplateError};
use crate::time::{Timestamp, UtcOffset};

/// 事实的模板：环境（`core/facts/env.txt`）、权限（`core/facts/permission.txt`）、会话编号
/// （`core/facts/session.txt`），和回复被出错打断的那一句（`core/facts/reply-cut.txt`）。
///
/// 由执行器从资源目录读好交进来，造会话时读一次，冻结在会话上（内核 K3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactTemplates {
    /// 环境那一块，字段是 `time`、`timezone`、`cwd`。
    env: Template,
    /// 权限那一块，字段是 `level`。
    permission: Template,
    /// 回复被出错打断的那一句，没有字段。
    reply_cut: Template,
    /// 会话编号那一块，字段是 `id`（施工 1-13 再补）。以前造的快照里没有这份模板，是没有：那些会话不注入这一块。
    session: Option<Template>,
}

/// 会话所在的环境：时区和工作目录（`02-内核.md` 第六节「回合怎么开、请求怎么发」第 6 条）。
///
/// 造会话时交进来，执行器报「环境变了」就换掉。时间不在这里，取边界上那条输入到的时刻。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// 所在的时区，取自执行器。
    pub offset: UtcOffset,
    /// 工作目录，头报上来的、人看到的那种写法，例如 `~/src/miyu`。内核不改写它。
    pub cwd: String,
    /// 加进来的目录：和工作区一样能读能写（施工 5-10 上）。照头报的原样，内核不改写，也不写进给她看的事实。
    pub dirs: Vec<String>,
}

impl FactTemplates {
    /// 读四个模板；会话编号那一份没有的（以前造的快照），不注入那一块。读好以后拿全部字段试着换一次：少了字段当场
    /// 报错，回合里就不会再出这种错。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了这一类没有的字段，返回 [`TemplateError`]，写明哪里坏了。
    pub fn new(
        env: &str,
        permission: &str,
        reply_cut: &str,
        session: Option<&str>,
    ) -> Result<FactTemplates, TemplateError> {
        let templates = FactTemplates {
            env: Template::parse(env)?,
            permission: Template::parse(permission)?,
            reply_cut: Template::parse(reply_cut)?,
            session: session.map(Template::parse).transpose()?,
        };
        templates.env.render(&env_fields("", "", ""))?;
        templates.permission.render(&permission_fields(""))?;
        templates.reply_cut.render(&BTreeMap::new())?;
        if let Some(session) = &templates.session {
            session.render(&session_fields(""))?;
        }
        Ok(templates)
    }

    /// 一个边界上要查的几块，照注入的先后：环境、权限、会话编号（没有这份模板的不查）。该注入哪几块，交给
    /// [`changed`] 比。时刻取边界上那条输入的，环境、权限、编号取会话现在的（`kernel/request.md`「事实」第 3、4 条）。
    pub fn boundary(
        &self,
        now: Timestamp,
        environment: &Environment,
        permission: &Permission,
        session: &SessionId,
    ) -> Vec<ContextInjected> {
        let mut facts = vec![self.env(now, environment), self.permission(permission)];
        facts.extend(self.session(session));
        facts
    }

    /// 回复被出错打断的那一块：跟在半截回复后面（施工 3-5 下）。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过；`reply_cut` 这个类别名也合写法。
    pub fn reply_cut(&self) -> ContextInjected {
        ContextInjected {
            kind: kind("reply_cut"),
            text: self
                .reply_cut
                .render(&BTreeMap::new())
                .expect("造的时候试换过，没有字段"),
        }
    }

    /// 环境那一块：此刻 `now` 到小时，时区，工作目录。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经拿全部字段试换过；`env` 这个类别名也合写法。
    pub fn env(&self, now: Timestamp, environment: &Environment) -> ContextInjected {
        let time = now.local_hour(environment.offset);
        let timezone = environment.offset.to_string();
        let fields = env_fields(&time, &timezone, &environment.cwd);
        ContextInjected {
            kind: kind("env"),
            text: self.env.render(&fields).expect("造的时候试换过，字段都有"),
        }
    }

    /// 会话编号那一块：这个会话自己的编号，子会话写它自己的（施工 1-13 再补）。以前造的快照没有这份模板，没有这一块。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经拿全部字段试换过；`session` 这个类别名也合写法。
    pub fn session(&self, id: &SessionId) -> Option<ContextInjected> {
        let template = self.session.as_ref()?;
        Some(ContextInjected {
            kind: kind("session"),
            text: template
                .render(&session_fields(id.as_str()))
                .expect("造的时候试换过，字段都有"),
        })
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

fn session_fields(id: &str) -> BTreeMap<&str, &str> {
    BTreeMap::from([("id", id)])
}

/// 内核自己用的类别名，都合写法。
fn kind(name: &str) -> FactKind {
    FactKind::parse(name).expect("内核自己用的类别名合写法")
}

#[cfg(test)]
mod tests;
