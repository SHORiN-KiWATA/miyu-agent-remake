//! 联想（施工 R-8，`docs/blueprint/memory.md` 第四条）：人开的一轮，回合开始的挂接点照这一轮人说的话找最像的几条记忆，过门槛
//! 的带上，宁可不带。
//!
//! - 照意思找（`Vectors::query`），门槛照模型（`core/memory/recall.toml`，[`Thresholds`]）；没有照意思那一路的、模型不在表里的
//!   不联想（2026-10-10 项目主人定）。关键词那一路不参与（`miyu_recall::associate`）。
//! - 只要现在算数的、出处活着的、听众合的；这一段上下文里模块 `memory` 交过的（常驻那一块的、之前联想带过的，照 `present`
//!   的 `refs`）不要。最多三条，一块不超过 [`LIMIT`] 字节；这一段联想累计带过 40 条的不再带（压缩以后从零数）。
//! - 一块 `context.injected`，模块 `memory`、类别 `recall`，外壳加一条一行（和常驻那一块、`memory_search` 的一行同一个写法）。

use std::collections::BTreeSet;
use std::path::Path;

use miyu_kernel::event::ContextInjected;
use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId};
use miyu_kernel::session::Injection;
use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use miyu_recall::associate::{Thresholds, pick};
use miyu_recall::{Entry, MemoryId};
use miyu_store::resources::ResourceRoot;
use miyu_tool::load::{self, LoadError};

use super::{Keeper, Query, Using};
use crate::blocking::blocking;

/// 注入那一块的模块：和常驻那一块同一个 `memory`。
const MODULE: &str = "memory";

/// 注入那一块的类别：`recall`。常驻那一块是 `memory`，`present` 照类别分得开。
const KIND: &str = "recall";

/// 一块最多几个字节（`memory.md` 第四条第 4 款：每轮约 150 token）：常驻那一块 3000 字节量过 964 token，照这个比例。
pub const LIMIT: usize = 500;

/// 人说的话最多拿前几个字去算向量：人可能贴进来一整个文件，模型本来也只看前 512 个 token（`recall.md` 第四条第 5 款）。
const SAID: usize = 1000;

/// 照向量找多少条再挑：交过的、作废的、听众不合的挑掉以后还够三条。
const WIDE: usize = 32;

/// 联想要的几样（`resources/software/memory/recall/` 的外壳，登记簿；一行照 `memory_search/memory.txt`；门槛表
/// `core/memory/recall.toml`，数据）。
#[derive(Debug, Clone)]
pub struct RecallTexts {
    open: Template,
    close: Template,
    line: Template,
    thresholds: Thresholds,
}

impl RecallTexts {
    /// 从资源目录 `resources` 读：外壳、一行、门槛表。
    ///
    /// # Errors
    ///
    /// 字读不出来、不是合写法的模板、要了别的字段；门槛表读不出来、写错了：交回为什么。
    pub fn load(resources: &ResourceRoot) -> Result<RecallTexts, String> {
        let thresholds = resources
            .memory_recall()
            .map_err(|error| error.to_string())
            .and_then(|text| {
                Thresholds::parse(&text).map_err(|why| format!("core/memory/recall.toml：{why}"))
            })?;
        RecallTexts::with(resources.path(), thresholds).map_err(|error| error.to_string())
    }

    /// 字照资源目录 `resources` 读，门槛照 `thresholds`（测试换成自己的模型）。
    ///
    /// # Errors
    ///
    /// 字读不出来、不是合写法的模板、要了别的字段。
    pub fn with(resources: &Path, thresholds: Thresholds) -> Result<RecallTexts, LoadError> {
        Ok(RecallTexts {
            open: load::text(resources, MODULE, "recall", "open", &[])?,
            close: load::text(resources, MODULE, "recall", "close", &[])?,
            line: load::text(
                resources,
                MODULE,
                "memory_search",
                "memory",
                &["id", "class", "date", "text"],
            )?,
            thresholds,
        })
    }
}

impl Keeper {
    /// 这一轮联想带什么（`said` 是人说的话，`offset` 是会话的时区，`present` 是内核交来的这一段上下文里模块注入过的）。人格
    /// 记忆没装的、核心没交联想的字的、没有照意思那一路的、模型不在门槛表里的、一条都没过门槛的，没有。
    pub(crate) async fn recall(
        &self,
        using: &Using,
        said: &str,
        offset: UtcOffset,
        present: &[Present],
    ) -> Option<Injection> {
        if !self.installed() {
            return None;
        }
        let texts = self.memory.recall()?;
        let said: String = said.chars().take(SAID).collect();
        let near = self.vectors()?.query(using, &said).await?;
        let threshold = texts.thresholds.of(&near.model)?;
        let (seen, brought) = seen(present);
        let keeper = self.clone();
        let found = match blocking(move || keeper.close(&near)).await {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!(target: crate::TARGET, error = error.as_str(), "memory recall not read");
                return None;
            }
        };
        let scored: Vec<(MemoryId, f32)> = found
            .iter()
            .map(|(entry, similar)| (entry.id, *similar))
            .collect();
        let picked = pick(&scored, threshold, &seen, brought);
        let entries: Vec<&Entry> = picked
            .iter()
            .filter_map(|id| {
                found
                    .iter()
                    .find(|(entry, _)| entry.id == *id)
                    .map(|(entry, _)| entry)
            })
            .collect();
        render(texts, &entries, offset)
    }

    /// 照问句的向量找最像的 [`WIDE`] 条，只留现在算数的、出处活着的、听众合的（最像的在前）。
    fn close(&self, near: &Query) -> Result<Vec<(Entry, f32)>, String> {
        let log = self.log().map_err(|refused| format!("{refused:?}"))?;
        let hits = log
            .index()
            .nearest(&near.model, &near.vector, WIDE)
            .map_err(|error| error.to_string())?;
        let found = log.book(|book| {
            hits.iter()
                .filter_map(|hit| {
                    let id = MemoryId::parse(&hit.key)?;
                    book.get(id).cloned().map(|entry| (entry, hit.similar))
                })
                .collect::<Vec<(Entry, f32)>>()
        });
        Ok(found
            .into_iter()
            .filter(|(entry, _)| self.shown(entry, false))
            .collect())
    }
}

/// 这一段上下文里模块 `memory` 交过的编号（常驻那一块的、联想带过的），和联想累计带过几条。
fn seen(present: &[Present]) -> (BTreeSet<MemoryId>, usize) {
    let ours = present
        .iter()
        .filter(|present| present.module.as_str() == MODULE);
    let seen = ours
        .clone()
        .flat_map(|present| present.refs.iter())
        .filter_map(|id| MemoryId::parse(id))
        .collect();
    let brought = ours
        .filter(|present| present.kind.as_str() == KIND)
        .map(|present| present.refs.len())
        .sum();
    (seen, brought)
}

/// 拼那一块：外壳加一条一行，照先后加，加上这一行就超过 [`LIMIT`] 字节的停；一条都放不下的没有。
fn render(texts: &RecallTexts, entries: &[&Entry], offset: UtcOffset) -> Option<Injection> {
    let say = |template: &Template, fields: &[(&str, &str)]| {
        load::say(template, fields).trim_end().to_string()
    };
    let (open, close) = (say(&texts.open, &[]), say(&texts.close, &[]));
    let mut used = open.len() + close.len() + 3;
    let (mut lines, mut refs) = (Vec::new(), Vec::new());
    for entry in entries {
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
        if used + row.len() + 1 > LIMIT {
            break;
        }
        used += row.len() + 1;
        lines.push(row);
        refs.push(id);
    }
    if lines.is_empty() {
        return None;
    }
    Some(Injection {
        module: ModuleId::parse(MODULE).expect("合写法"),
        fact: ContextInjected {
            kind: FactKind::parse(KIND).expect("合写法"),
            text: format!("{open}\n{}\n{close}\n", lines.join("\n")),
            refs,
        },
    })
}

#[cfg(test)]
mod tests;
