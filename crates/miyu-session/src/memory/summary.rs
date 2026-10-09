//! 常驻的记忆摘要（施工 R-4 上，`docs/blueprint/memory.md` 第三条）：会话的第一轮、压缩以后的第一轮，回合开始的挂接点交回
//! 一块事实，在触发的那句前面，以后原样回放（缓存只在第一轮多写一次，第十条）。
//!
//! - 交不交：内核交来的 `present` 里没有这个模块、这一类的一块才交（第一轮、压缩以后、撤掉带着它的那一轮以后）；人格记忆这时
//!   没装的不交（施工 R-10）。
//! - 交什么：这一间里现在算数的、出处活着的、听众合的，新的在前，一条一行，和 `memory_search` 的一行一个写法；`refs` 是这几条
//!   的编号。排名（半衰期、用到几次）随 R-4 下；合并出来的摘要随 R-7。
//! - 上限：外壳加几行不超过 [`LIMIT`] 字节，截在一条的边界上，最后一行说还有几条。

use std::path::Path;

use miyu_kernel::event::ContextInjected;
use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId};
use miyu_kernel::session::Injection;
use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use miyu_tool::load::{self, LoadError};

use super::{Filter, Keeper};

/// 记忆这个软件包在资源目录里的名字（和 `miyu-memory` 的同一个；会话这一层不依赖软件包）。
const PACKAGE: &str = "memory";

/// 注入那一块的模块和类别：都叫 `memory`。
const NAME: &str = "memory";

/// 一块最多几个字节（`memory.md` 第三条第 3 款）：起点是约 1000 token。中文为主的一块照字节/4 折算会少算，2026-10-08 在
/// 开发端点量过：4000 字节 1298 token，3000 字节 964 token，所以取 3000。
pub const LIMIT: usize = 3000;

/// 摘要那一块给模型看的字：外壳、一行（和 `memory_search` 的同一份）、截了的那一句（`resources/software/memory/`，登记簿）。
#[derive(Debug, Clone)]
pub struct SummaryTexts {
    open: Template,
    line: Template,
    more: Template,
    close: Template,
}

impl SummaryTexts {
    /// 从资源目录 `resources` 读：`summary/open.txt`、`summary/close.txt`、`summary/more.txt`（`{count}`），一行照
    /// `memory_search/memory.txt`（`{id}`、`{class}`、`{date}`、`{text}`）。
    ///
    /// # Errors
    ///
    /// 读不出来；不是合写法的模板，或者要了别的字段。
    pub fn load(resources: &Path) -> Result<SummaryTexts, LoadError> {
        Ok(SummaryTexts {
            open: load::text(resources, PACKAGE, "summary", "open", &[])?,
            line: load::text(
                resources,
                PACKAGE,
                "memory_search",
                "memory",
                &["id", "class", "date", "text"],
            )?,
            more: load::text(resources, PACKAGE, "summary", "more", &["count"])?,
            close: load::text(resources, PACKAGE, "summary", "close", &[])?,
        })
    }
}

impl Keeper {
    /// 这一轮要不要交摘要、交什么（这一段上下文里已经有的、一条都没有的、读不了的、核心没读到外壳的字的不交）。`offset`
    /// 是会话的时区。碰磁盘，调的一方放在阻塞线程里。
    pub(crate) fn summary(&self, offset: UtcOffset, present: &[Present]) -> Option<Injection> {
        if !self.installed() {
            return None;
        }
        let name =
            |present: &Present| present.module.as_str() == NAME && present.kind.as_str() == NAME;
        let texts = self.memory.summary.as_ref()?;
        if present.iter().any(name) {
            return None;
        }
        let filter = Filter {
            class: None,
            from: None,
            forgotten: false,
            limit: usize::MAX,
        };
        let entries = match self.list(&filter) {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(target: crate::TARGET, error = error.as_str(), "memory summary not read");
                return None;
            }
        };
        if entries.is_empty() {
            return None;
        }
        let say = |template: &Template, fields: &[(&str, &str)]| {
            load::say(template, fields).trim_end().to_string()
        };
        let (open, close) = (say(&texts.open, &[]), say(&texts.close, &[]));
        // 截了的那一句照最长的数留出地方：一条都放不下也说得出还有几条。
        let more = say(&texts.more, &[("count", &entries.len().to_string())]);
        let room = LIMIT.saturating_sub(open.len() + close.len() + more.len() + 3);
        let mut lines = Vec::new();
        let mut used = 0;
        let mut refs = Vec::new();
        for entry in &entries {
            let (id, date) = (entry.id.to_string(), entry.at.local_date(offset));
            let text = entry.text.replace('\n', " ");
            let row = say(
                &texts.line,
                &[
                    ("id", &id),
                    ("class", &entry.class),
                    ("date", &date),
                    ("text", &text),
                ],
            );
            if used + row.len() + 1 > room {
                break;
            }
            used += row.len() + 1;
            lines.push(row);
            refs.push(id);
        }
        let left = entries.len() - lines.len();
        if left > 0 {
            lines.push(say(&texts.more, &[("count", &left.to_string())]));
        }
        let text = format!("{open}\n{}\n{close}\n", lines.join("\n"));
        Some(Injection {
            module: ModuleId::parse(NAME).expect("合写法"),
            fact: ContextInjected {
                kind: FactKind::parse(NAME).expect("合写法"),
                text,
                refs,
            },
        })
    }
}
