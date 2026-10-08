//! 环境和状态的事实：写成什么、该不该注入（`docs/designs/08-上下文投影.md` 第五节 C10、
//! 「环境和状态的事实怎么写」）。
//!
//! 现在有三类，都由内核注入，一类一块：环境（`env`：时间、时区、工作目录）、权限级别
//! （`permission`）和会话编号（`session`，施工 1-13 再补）。每个边界查一遍：每一块和有效历史里同一个来源、
//! 同一类的最近一块比，环境、编号逐字节相同就不注入（[`changed`]），权限比级别（下面）。在哪些边界查，是回合状态机和
//! 压缩的事（施工 2-3、2-7、M6）。
//! 会话编号一个会话里不变，所以只在第一轮、压缩以后、撤掉带着它的那一轮以后注入；它不并进环境那一块：环境每过整点
//! 重发，编号跟着重发就白花。
//!
//! 权限那一块比的是级别，不是原文（施工 2-7 补）：人切了级别，下一个边界上有她看到过的上一块的，用切换那一份模板写，
//! 带上上一块的级别，她看得出这是人切的、从哪一级切过来；没有上一块的（第一轮、压缩以后、撤掉了带着它们的几轮以后）照旧
//! 用平常那一份。
//!
//! 还有一块不是环境和状态，是发生了的事：回复被出错打断了（`reply_cut`，施工 3-5 下）。它跟在
//! 半截回复后面，每次都注入，不和以前的比。
//!
//! 有角色扮演提示的人格还有一块 `reminder`（施工 P-1 补，`08-上下文投影.md` C3、`26-提示词.md` J10）：只在回合开始查，
//! 隔几轮一条，不比原文。组装把回合开始时注入的这一块排在触发的那句后面（C2 唯一的例外）。

use std::collections::BTreeMap;

use crate::event::{Body, ContextInjected, Level, Permission};
use crate::history::History;
use crate::id::{FactKind, ModuleId, SessionId};
use crate::origin::By;
use crate::template::{Template, TemplateError};
use crate::time::{Timestamp, UtcOffset};

/// 事实的模板：环境（`core/facts/env.txt`）、权限（`core/facts/permission.txt`）、会话编号
/// （`core/facts/session.txt`）、切了级别以后的权限（`core/facts/permission-changed.txt`），和回复被出错打断的那一句
/// （`core/facts/reply-cut.txt`）。
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
    /// 切了级别以后的权限那一块，字段是 `level`、`previous`（施工 2-7 补）。以前造的快照里没有这份模板，是没有：那些会话
    /// 切了照旧用 `permission` 那一份，前缀一字不变。
    permission_changed: Option<Template>,
    /// 角色扮演提示那一块的原文，拼好的（施工 P-1 补）。没有的不注入。
    reminder: Option<String>,
}

/// 角色扮演提示这一类的名字。组装照它把回合开始时注入的这一块排在触发的那句后面（`08-上下文投影.md` C2 唯一的例外）。
pub const REMINDER: &str = "reminder";

/// 角色扮演提示隔几轮一条：连这一轮数，到这个数注入（`08-上下文投影.md` C3，旧版实测的间隔）。
pub const REMINDER_EVERY: usize = 3;

/// 权限那一块写得出的几级，写法照 [`effective_level`]。认上一块说的是哪一级时，照这个先后试。
const LEVELS: [&str; 3] = ["read_only", "workspace", "full"];

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
    /// 读五个模板；会话编号、切换那两份没有的（以前造的快照），不注入会话编号那一块，切了级别照旧用权限那一份。读好以后
    /// 拿全部字段试着换一次：少了字段当场报错，回合里就不会再出这种错。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了这一类没有的字段，返回 [`TemplateError`]，写明哪里坏了。
    pub fn new(
        env: &str,
        permission: &str,
        reply_cut: &str,
        session: Option<&str>,
        permission_changed: Option<&str>,
    ) -> Result<FactTemplates, TemplateError> {
        let templates = FactTemplates {
            env: Template::parse(env)?,
            permission: Template::parse(permission)?,
            reply_cut: Template::parse(reply_cut)?,
            session: session.map(Template::parse).transpose()?,
            permission_changed: permission_changed.map(Template::parse).transpose()?,
            reminder: None,
        };
        templates.env.render(&env_fields("", "", ""))?;
        templates.permission.render(&permission_fields(""))?;
        templates.reply_cut.render(&BTreeMap::new())?;
        if let Some(session) = &templates.session {
            session.render(&session_fields(""))?;
        }
        if let Some(changed) = &templates.permission_changed {
            changed.render(&changed_fields("", ""))?;
        }
        Ok(templates)
    }

    /// 一个边界上该注入的几块，照注入的先后：环境、权限、会话编号（没有这份模板的不查）。每一块和有效历史里内核记的同一类
    /// 最近一块比：环境、编号比原文，一样的不注入（[`changed`]）；权限比级别，变了的照有没有上一块挑模板（施工 2-7 补）。
    /// 时刻取边界上那条输入的，环境、权限、编号取会话现在的（`kernel/request.md`「事实」第 2 到 4 条）。
    pub fn boundary(
        &self,
        history: &History,
        now: Timestamp,
        environment: &Environment,
        permission: &Permission,
        session: &SessionId,
    ) -> Vec<ContextInjected> {
        let fresh = |fact: ContextInjected| unseen(history, &By::Kernel, &fact).then_some(fact);
        [
            fresh(self.env(now, environment)),
            self.permission_due(history, permission),
            self.session(session).and_then(fresh),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// 带上角色扮演提示那一块的原文（施工 P-1 补）：造策略时拼好交进来，冻结在会话上。`None` 的不注入。
    #[must_use]
    pub fn with_reminder(mut self, text: Option<String>) -> FactTemplates {
        self.reminder = text;
        self
    }

    /// 回合开始时要不要注入角色扮演提示（施工 P-1 补，`kernel/request.md`「事实」第 1 条）：有效历史里没有内核记的这一块，
    /// 注入；有的，数它后面开了几轮（这一轮的 `turn.started` 已经记下），到 [`REMINDER_EVERY`] 注入。压缩替掉的、撤掉的
    /// 不在有效历史里，下一轮重来。不比原文：同一段字隔几轮再说一次，就是它的用处。
    pub fn reminder(&self, history: &History) -> Option<ContextInjected> {
        let text = self.reminder.as_ref()?;
        let kind = kind(REMINDER);
        let mut turns = 0;
        for event in history.events().iter().rev() {
            match &event.body {
                Body::TurnStarted(_) => turns += 1,
                Body::ContextInjected(fact) if event.by == By::Kernel && fact.kind == kind => {
                    if turns < REMINDER_EVERY {
                        return None;
                    }
                    break;
                }
                _ => {}
            }
        }
        Some(ContextInjected {
            refs: Vec::new(),
            kind,
            text: text.clone(),
        })
    }

    /// 回复被出错打断的那一块：跟在半截回复后面（施工 3-5 下）。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过；`reply_cut` 这个类别名也合写法。
    pub fn reply_cut(&self) -> ContextInjected {
        ContextInjected {
            refs: Vec::new(),
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
            refs: Vec::new(),
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
            refs: Vec::new(),
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
    /// 实际不会 panic：`permission` 这个类别名合写法。
    pub fn permission(&self, permission: &Permission) -> ContextInjected {
        ContextInjected {
            refs: Vec::new(),
            kind: kind("permission"),
            text: self.permission_text(effective_level(permission)),
        }
    }

    /// 切了级别以后的权限那一块：实际生效的那一级，和她在上一块里看到的那一级 `previous`（写法照 [`effective_level`]，
    /// 施工 2-7 补）。以前造的快照没有这份模板，没有这一块。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：`permission` 这个类别名合写法。
    pub fn permission_changed(
        &self,
        permission: &Permission,
        previous: &str,
    ) -> Option<ContextInjected> {
        let template = self.permission_changed.as_ref()?;
        Some(ContextInjected {
            refs: Vec::new(),
            kind: kind("permission"),
            text: changed_text(template, effective_level(permission), previous),
        })
    }

    /// 权限那一块该不该注入、写成什么（施工 2-7 补，`kernel/request.md`「事实」第 2 条）。和有效历史里内核记的最近一块
    /// 比的是级别，不是原文：切换那一份带着上一级，和同一级的平常那一份原文不一样。
    ///
    /// - 最近那一块说的就是这一级：不注入。来回切了一圈的，也就不注入。
    /// - 没有上一块（第一轮，压缩以后，撤掉了带着它们的那几轮以后）：平常那一份。
    /// - 有上一块、级别不一样：切换那一份，`previous` 写上一块的级别。没有这份模板的（以前造的快照）写平常那一份，和以前
    ///   逐字节比原文一模一样。
    fn permission_due(
        &self,
        history: &History,
        permission: &Permission,
    ) -> Option<ContextInjected> {
        let plain = self.permission(permission);
        let Some(last) = latest(history, &By::Kernel, &plain.kind) else {
            return Some(plain);
        };
        if self.says(last, effective_level(permission)) {
            return None;
        }
        LEVELS
            .into_iter()
            .find(|previous| self.says(last, previous))
            .and_then(|previous| self.permission_changed(permission, previous))
            .or(Some(plain))
    }

    /// 权限那一块的原文 `text` 说的是不是 `level` 这一级：平常那一份写出来的，或者切换那一份从随便哪一级切过来写出来的。
    /// 模板冻结在会话上（内核 K3），她看到过的每一块都是这几种写法之一；认不出的当作不是，照平常那一份注入，和以前逐字节
    /// 比一样。
    fn says(&self, text: &str, level: &str) -> bool {
        self.permission_text(level) == text
            || self.permission_changed.as_ref().is_some_and(|template| {
                LEVELS
                    .into_iter()
                    .any(|previous| changed_text(template, level, previous) == text)
            })
    }

    /// 平常那一份写出 `level` 这一级。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经拿全部字段试换过。
    fn permission_text(&self, level: &str) -> String {
        self.permission
            .render(&permission_fields(level))
            .expect("造的时候试换过，字段都有")
    }
}

/// 切换那一份写出从 `previous` 切到 `level`。
///
/// # Panics
///
/// 实际不会 panic：造的时候已经拿全部字段试换过。
fn changed_text(template: &Template, level: &str, previous: &str) -> String {
    template
        .render(&changed_fields(level, previous))
        .expect("造的时候试换过，字段都有")
}

/// 这几块里该注入的，照原来的先后：和有效历史里同一个来源、同一类的最近一块比，
/// 逐字节相同的去掉（08 C10）。
///
/// 比的是最近的那一块，不是随便哪一块：先是 A，一个边界变成 B，下一个边界又回到 A，
/// 她最近看到的是 B，A 要重新注入。压缩替掉的、撤销掉的不在有效历史里，也就不算。
///
/// 边界上的权限那一块不走它，比的是级别（[`FactTemplates::boundary`]，施工 2-7 补）。
pub fn changed(history: &History, by: &By, facts: Vec<ContextInjected>) -> Vec<ContextInjected> {
    facts
        .into_iter()
        .filter(|fact| unseen(history, by, fact))
        .collect()
}

/// 这一块和有效历史里同一个来源、同一类的最近一块不一样，或者那一类还没有过。
fn unseen(history: &History, by: &By, fact: &ContextInjected) -> bool {
    latest(history, by, &fact.kind) != Some(fact.text.as_str())
}

/// 有效历史里由模块注入的一块（施工 R-4 上）：回合开始交给挂接点（`Action::RunTurnStartHooks` 的 `present`），模块照它定
/// 这一段上下文里还要不要交。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Present {
    /// 哪个模块注入的。
    pub module: ModuleId,
    /// 这一块的类别。
    pub kind: FactKind,
    /// 这一块带着的编号（[`ContextInjected::refs`]），原样。
    pub refs: Vec<String>,
}

/// 有效历史里由模块注入的 `context.injected`，照日志的先后（施工 R-4 上）。和会话编号那一块同一个找法：最近一个检查点
/// 以后、撤掉的回合不算（[`History`] 就是这样的）；内核自己注入的不算。
pub fn present(history: &History) -> Vec<Present> {
    history
        .events()
        .iter()
        .filter_map(|event| match (&event.by, &event.body) {
            (By::Module(module), Body::ContextInjected(fact)) => Some(Present {
                module: module.id.clone(),
                kind: fact.kind.clone(),
                refs: fact.refs.clone(),
            }),
            _ => None,
        })
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
/// 按最严的算，告诉她的也是最严的那一级。`history` 列切权限的那一条也照它写（施工 2-7 补）。
pub fn effective_level(permission: &Permission) -> &'static str {
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

fn changed_fields<'a>(level: &'a str, previous: &'a str) -> BTreeMap<&'a str, &'a str> {
    BTreeMap::from([("level", level), ("previous", previous)])
}

/// 内核自己用的类别名，都合写法。
fn kind(name: &str) -> FactKind {
    FactKind::parse(name).expect("内核自己用的类别名合写法")
}

#[cfg(test)]
mod tests;
