//! 合并（施工 R-7 上，`docs/blueprint/memory.md` 第七条）：抽取只往里加，隔一段时间、攒够几个会话，把新记的和相关的旧记忆放在
//! 一起理一遍：重复的、被推翻的作废，相对的日期改成绝对的，写一段新的摘要。
//!
//! - **什么时候**：抽取记下以后看一眼（`extract.rs` 的 `Job`）：上次合并以后有新记的（现在算数、听众合、出处活着，不算合并
//!   自己改出来的）；那以后抽过的会话够 `memory.merge_sessions`；离上次合并过了 `memory.merge_every`（没合并过的不看时间）。
//!   一间一把锁（[`Merges`]），在后台合，不挡会话。
//! - **交什么**：一条 user，指令在前，后面是现在的摘要、上次合并以后新记的（旧的在前）、每条新记的照关键词和意思搜出来的相关
//!   旧记忆（每条最多 [`RELATED`] 条，去重）。一条一行，同 `memory_search` 的写法。最多 [`ROOM`] 字节：放不下的合完马上接着
//!   合剩下的。发出去以前遮 key。
//! - **交回、怎么落**：读法在 `miyu_recall::merge`；改的指着旧的（类、出处、听众照旧）、作废的带为什么、摘要、合到哪的记号，
//!   都由记忆模块记。落的时候还算数、听众合的才落。
//! - **出错**：不动真相，下次抽完再来；同一间连着 [`TRIES`] 次不成的，记一条失败的记号、跳过这一批（成了一次、跳过一次都重新
//!   数：这一批从哪起只在这两种时候变）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{AccountId, Seq};
use miyu_kernel::request::Message;
use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use miyu_recall::merge::decisions;
use miyu_recall::redact::redact;
use miyu_recall::{Entry, MemoryId};
use miyu_store::recall::Room;
use miyu_tool::TEXT_CHARS;
use miyu_tool::load::{self, LoadError, say};

use super::extract::{ROOM, secrets};
use super::{Extraction, Keeper, Using};
use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::config::TurnConfig;
use crate::route::Ask;
use crate::settings::MemorySettings;

mod land;

/// 每条新记的最多带几条相关的旧记忆。
pub(crate) const RELATED: usize = 3;

/// 摘要最多几个字：常驻的那一块 3000 字节里，摘要占一半上下（中文一个字三个字节），留地方给之后新记的（R-7 下）。
pub(crate) const SUMMARY_CHARS: usize = 600;

/// 同一批连着合不成几次就跳过。
const TRIES: u32 = 3;

/// 用途：记运行日志、记账（和抽取的同一个）。
const PURPOSE: &str = "memory";

/// 记忆这个软件包在资源目录里的名字（和 `extract.rs` 的同一个）。
const PACKAGE: &str = "memory";

/// 合并给模型看的字（`resources/software/memory/merge/`，登记簿）：指令、现在的摘要、还没有摘要、两块的标题，一条一行照
/// `memory_search/memory.txt`。
#[derive(Debug, Clone)]
pub struct MergeTexts {
    instruction: Template,
    summary: Template,
    none: Template,
    new: Template,
    related: Template,
    line: Template,
}

impl MergeTexts {
    /// 从资源目录 `resources` 读：`merge/instruction.txt`、`summary.txt`（`{text}`）、`none.txt`、`new.txt`、`related.txt`，一行
    /// 照 `memory_search/memory.txt`（`{id}`、`{class}`、`{date}`、`{text}`）。
    ///
    /// # Errors
    ///
    /// 哪一份读不出来、写法不对、要了别的字段。
    pub fn load(resources: &Path) -> Result<MergeTexts, LoadError> {
        let text =
            |name: &str, fields: &[&str]| load::text(resources, PACKAGE, "merge", name, fields);
        Ok(MergeTexts {
            instruction: text("instruction", &[])?,
            summary: text("summary", &["text"])?,
            none: text("none", &[])?,
            new: text("new", &[])?,
            related: text("related", &[])?,
            line: load::text(
                resources,
                PACKAGE,
                "memory_search",
                "memory",
                &["id", "class", "date", "text"],
            )?,
        })
    }
}

/// 是合并自己改出来的（`by` 是记忆模块、写了 `replaces`）：合并不当它是新记的，常驻的那一块有摘要时不列它（施工 R-7 下）。
pub(super) fn made_by_merge(entry: &Entry) -> bool {
    entry.replaces.is_some()
        && matches!(&entry.by, miyu_kernel::origin::By::Module(module) if module.id.as_str() == super::MODULE)
}

/// 核心一份的合并状态：哪几间在合、每一间同一批失败了几次。
#[derive(Debug, Default)]
pub(crate) struct Merges {
    state: Mutex<MergeState>,
}

#[derive(Debug, Default)]
struct MergeState {
    running: BTreeSet<Room>,
    /// 每一间连着失败了几次。
    failures: BTreeMap<Room, u32>,
}

impl Merges {
    fn state(&self) -> std::sync::MutexGuard<'_, MergeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 开始合 `room`：已经有一个在合的交回 `false`。
    fn start(&self, room: &Room) -> bool {
        self.state().running.insert(room.clone())
    }

    /// 合完了（成没成都算）。
    fn finish(&self, room: &Room) {
        self.state().running.remove(room);
    }

    /// 这一次成没成：成了的清掉失败的次数，交回 0；没成的交回连着第几次，到了 [`TRIES`] 次的也清掉（这一批跳过了）。
    fn record(&self, room: &Room, ok: bool) -> u32 {
        let mut state = self.state();
        if ok {
            state.failures.remove(room);
            return 0;
        }
        let tries = state.failures.get(room).copied().unwrap_or(0) + 1;
        if tries >= TRIES {
            state.failures.remove(room);
        } else {
            state.failures.insert(room.clone(), tries);
        }
        tries
    }
}

/// 合并要的几样：抽取那一次的（字、一次性入口、key 的写法）、这一轮的配置、记在谁的账上、会话的时区。
pub(crate) struct MergeJob {
    /// 哪一间、听众是谁。
    pub(crate) keeper: Keeper,
    /// 字、一次性入口、key 的写法。
    pub(crate) extraction: Extraction,
    /// 这一轮的配置：照它取 key、发、读 `memory.merge_*`。
    pub(crate) config: TurnConfig,
    /// 记在谁的账上：会话的属主。
    pub(crate) owner: AccountId,
    /// 日期照会话的时区写。
    pub(crate) offset: UtcOffset,
}

/// 一批：现在的摘要、上次合并以后新记的（旧的在前）。
struct Batch {
    summary: Option<String>,
    fresh: Vec<Entry>,
}

/// 交出去的一次：请求的字、交进去的编号、合到第几条。
struct Asked {
    text: String,
    given: BTreeSet<MemoryId>,
    upto: Seq,
}

impl MergeJob {
    /// 抽取记下以后调：够了的在后台合（`memory.md` 第七条第 1 款）。同一间已经在合的、不够的什么都不做。
    pub(crate) fn start(self) {
        tokio::spawn(async move {
            let room = self.keeper.room().clone();
            if !self.keeper.memory.merges.start(&room) {
                return;
            }
            self.run().await;
            self.keeper.memory.merges.finish(&room);
        });
    }

    /// 合到没有剩下的：头一批看够不够，接着合的只看还有没有新记的（放不下的、合着的时候又记的）。
    async fn run(&self) {
        let settings = MemorySettings::from(&self.config.resolved.values());
        let sessions = usize::try_from(settings.merge_sessions).unwrap_or(usize::MAX);
        let mut check = Some((settings.merge_every, sessions));
        loop {
            let keeper = self.keeper.clone();
            let found = blocking(move || keeper.merge_batch(wall_now(), check)).await;
            let batch = match found {
                Ok(Some(batch)) => batch,
                Ok(None) => return,
                Err(refused) => {
                    tracing::warn!(target: TARGET, room = %self.keeper.room(), error = ?refused, "memory merge failed");
                    return;
                }
            };
            check = None;
            let asked = self.compose(batch).await;
            if !self.ask(&asked).await {
                return;
            }
        }
    }

    /// 拼这一次：新记的照先后一条条加，每条带上相关的旧记忆，放不下的留给下一次（一条都放不下也至少交一条）。
    async fn compose(&self, batch: Batch) -> Asked {
        let texts = &self.extraction.merge;
        let row = |entry: &Entry| {
            say(
                &texts.line,
                &[
                    ("id", &entry.id.to_string()),
                    ("class", &entry.class),
                    ("date", &entry.at.local_date(self.offset)),
                    ("text", &entry.text.replace('\n', " ")),
                ],
            )
        };
        let summary = match &batch.summary {
            Some(text) => say(&texts.summary, &[("text", text)]),
            None => say(&texts.none, &[]),
        };
        let head = format!(
            "{}\n\n{}\n\n{}",
            say(&texts.instruction, &[]).trim_end(),
            summary.trim_end(),
            say(&texts.new, &[]).trim_end()
        );
        let related_head = say(&texts.related, &[]).trim_end().to_string();
        let fresh_ids: BTreeSet<MemoryId> = batch.fresh.iter().map(|entry| entry.id).collect();
        let (mut new_rows, mut old_rows) = (Vec::new(), Vec::new());
        let mut given = BTreeSet::new();
        let mut bytes = head.len() + related_head.len() + 2;
        let mut upto = None;
        for entry in &batch.fresh {
            let related = self.related(entry, &fresh_ids, &given).await;
            let added: usize = std::iter::once(entry)
                .chain(&related)
                .map(|one| row(one).len() + 1)
                .sum();
            if upto.is_some() && bytes + added > ROOM {
                break;
            }
            bytes += added;
            new_rows.push(row(entry));
            given.insert(entry.id);
            for one in &related {
                old_rows.push(row(one));
                given.insert(one.id);
            }
            upto = Some(entry.id.seq());
        }
        let upto = upto.unwrap_or(Seq::FIRST);
        let mut text = format!("{head}\n{}", new_rows.join("\n"));
        if !old_rows.is_empty() {
            text.push_str(&format!("\n\n{related_head}\n{}", old_rows.join("\n")));
        }
        Asked { text, given, upto }
    }

    /// `entry` 的相关旧记忆：照关键词和意思搜（有向量那一路的），不算新记的、已经交进去的，最多 [`RELATED`] 条。
    async fn related(
        &self,
        entry: &Entry,
        fresh: &BTreeSet<MemoryId>,
        given: &BTreeSet<MemoryId>,
    ) -> Vec<Entry> {
        let near = match self.keeper.vectors() {
            Some(vectors) => {
                let using = Using {
                    config: Arc::clone(&self.config),
                    owner: self.owner.clone(),
                };
                vectors.query(&using, &entry.text).await
            }
            None => None,
        };
        let (keeper, text) = (self.keeper.clone(), entry.text.clone());
        let limit = RELATED + fresh.len();
        let found = blocking(move || keeper.search(&text, false, limit, near.as_ref())).await;
        match found {
            Ok(found) => found
                .into_iter()
                .filter(|one| !fresh.contains(&one.id) && !given.contains(&one.id))
                .take(RELATED)
                .collect(),
            Err(error) => {
                tracing::warn!(target: TARGET, room = %self.keeper.room(), error = error.as_str(), "memory merge search failed");
                Vec::new()
            }
        }
    }

    /// 发、读、遮、记；交回记成了没有。
    async fn ask(&self, asked: &Asked) -> bool {
        let started = Instant::now();
        let room = self.keeper.room().to_string();
        let secrets = secrets(&self.config);
        let shapes = &self.extraction.shapes;
        let organizer = MemorySettings::from(&self.config.resolved.values()).organizer;
        tracing::info!(target: TARGET, room = room.as_str(), given = asked.given.len(), "memory merge started");
        let ask = Ask {
            model: organizer,
            purpose: PURPOSE.to_string(),
            system: String::new(),
            messages: vec![Message::User {
                blocks: vec![Block::Text(Text {
                    text: redact(&asked.text, &secrets, shapes),
                })],
            }],
            max_tokens: None,
            owner: self.owner.clone(),
        };
        let answered = self
            .extraction
            .ask
            .call(&self.config, &self.extraction.blobs, ask)
            .await
            .map_err(|unanswered| unanswered.reason().to_string())
            .and_then(|answer| decisions(&answer.text, &asked.given, TEXT_CHARS, SUMMARY_CHARS));
        let merges = &self.keeper.memory.merges;
        let given = u32::try_from(asked.given.len()).unwrap_or(u32::MAX);
        match answered {
            Ok(mut decided) => {
                for revised in &mut decided.revised {
                    revised.text = redact(&revised.text, &secrets, shapes);
                }
                for retire in &mut decided.retired {
                    retire.why = redact(&retire.why, &secrets, shapes);
                }
                if let Some(summary) = &mut decided.summary {
                    *summary = redact(summary, &secrets, shapes);
                }
                let (keeper, upto) = (self.keeper.clone(), asked.upto);
                let recorded =
                    blocking(move || keeper.record_merge(wall_now(), &decided, upto, given)).await;
                merges.record(self.keeper.room(), true);
                match recorded {
                    Ok((revised, retired)) => {
                        let took_ms =
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                        tracing::info!(target: TARGET, room = room.as_str(), revised, retired, took_ms, "memory merged");
                        true
                    }
                    Err(refused) => {
                        tracing::warn!(target: TARGET, room = room.as_str(), error = ?refused, "memory merge not recorded");
                        false
                    }
                }
            }
            Err(why) => {
                let tries = merges.record(self.keeper.room(), false);
                tracing::warn!(target: TARGET, room = room.as_str(), tries, error = why.as_str(), "memory merge failed");
                if tries >= TRIES {
                    let (keeper, upto) = (self.keeper.clone(), asked.upto);
                    let skipped =
                        blocking(move || keeper.record_merge_failed(wall_now(), upto, given)).await;
                    if let Err(refused) = skipped {
                        tracing::warn!(target: TARGET, room = room.as_str(), error = ?refused, "memory merge not recorded");
                    }
                }
                false
            }
        }
    }
}
