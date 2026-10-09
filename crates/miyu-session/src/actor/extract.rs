//! 什么时候抽、这一段从哪来（施工 R-6 上，`docs/blueprint/memory.md` 第六条第 1 款；抽本身在 `memory/extract.rs`）。
//!
//! 1. 会话从忙到闲那一刻（`watchers.rs` 的 `after_batch`）上一个闹钟，`memory.extract_idle` 以后响；忙起来就撤掉。只给记忆
//!    开着、核心交过抽取要的几样的会话（本机的主会话：别的没有记忆的端口）。
//! 2. 响了、还闲着、没有一次在路上：在阻塞线程里读记忆日志里它抽到了哪、会话日志里那以后的事件（不占 actor，不碰 actor 手里
//!    的日志）。
//! 3. 读回来、还是那个闹钟、还闲着：拼一份留着一切的历史（`History::whole`，压缩替代掉的也在），交给会话的组装器渲染这一段，
//!    定怎么办，派出去。读的时候来了新一轮的这一次作废，等下次闲了再来。

use std::sync::Arc;

use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use tracing::Instrument;

use super::Actor;
use super::back::answer_back;
use crate::memory::{Job, plan};
use crate::port::Back;
use crate::settings::MemorySettings;

impl Actor {
    /// 刚从忙到闲：抽取开着的上一个闹钟。
    pub(super) fn extract_when_idle(&self) {
        let Some((calls, _)) = self.tools.memory() else {
            return;
        };
        let (keeper, _, _, extractor) = calls.extracting();
        if keeper.extraction().is_none() {
            return;
        }
        let idle = keeper
            .extraction()
            .and_then(|extraction| extraction.idle)
            .unwrap_or_else(|| {
                MemorySettings::from(&self.config.current().resolved.values()).extract_idle
            });
        let generation = extractor.arm();
        let backs = self.backs.clone();
        tokio::spawn(async move {
            tokio::time::sleep(idle).await;
            answer_back(&backs, Back::ExtractDue { generation });
        });
    }

    /// 忙起来了：撤掉闹钟。在路上的那一次照样走完。
    pub(super) fn extract_cancel(&self) {
        if let Some((calls, _)) = self.tools.memory() {
            calls.extracting().3.cancel();
        }
    }

    /// 第 `generation` 个闹钟响了：还算数、还闲着的，在阻塞线程里读这一段。
    pub(super) fn extract_due(&self, generation: u64) {
        let Some((calls, _)) = self.tools.memory() else {
            return;
        };
        let (keeper, session, _, extractor) = calls.extracting();
        let dir = self.store.as_ref().and_then(|store| store.dir());
        let Some(dir) = dir else {
            return;
        };
        if !extractor.due(generation) || !self.session.vacant() {
            return;
        }
        let (keeper, session, backs) = (keeper.clone(), session.clone(), self.backs.clone());
        tokio::task::spawn_blocking(move || {
            let read = keeper
                .extracted(&session)
                .map_err(|refused| format!("{refused:?}"))
                .and_then(|after| {
                    let events =
                        miyu_store::log::read_events(&dir).map_err(|error| error.to_string())?;
                    Ok((after.unwrap_or(Seq::FIRST), events))
                });
            answer_back(&backs, Back::ExtractRead { generation, read });
        });
    }

    /// 读回来了：还是那个闹钟、还闲着的，渲染这一段、定怎么办、派出去。
    pub(super) fn extract_read(&self, generation: u64, read: Result<(Seq, Vec<Event>), String>) {
        let Some((calls, offset)) = self.tools.memory() else {
            return;
        };
        let (keeper, session, owner, extractor) = calls.extracting();
        let Some(extraction) = keeper.extraction() else {
            return;
        };
        if !self.session.vacant() || !extractor.start(generation) {
            return;
        }
        let (after, events) = match read {
            Ok(read) => read,
            Err(error) => {
                tracing::warn!(target: crate::TARGET, error = error.as_str(), "memory extraction failed");
                extractor.abandon();
                return;
            }
        };
        let mut history = History::whole();
        for event in events {
            history.append(event);
        }
        let spoken = self.session.spoken_in(&history, after);
        let called = history.called_since(after);
        let config = Arc::clone(self.config.current());
        let settings = MemorySettings::from(&config.resolved.values());
        let min_turns = usize::try_from(settings.extract_turns).unwrap_or(1);
        let plan = plan(&spoken, &called, min_turns, offset, &extraction.texts);
        let job = Job {
            keeper: keeper.clone(),
            extraction,
            config,
            owner: owner.clone(),
            session: session.clone(),
            after,
            extractor: Arc::clone(extractor),
            again: {
                let (extractor, backs) = (Arc::clone(extractor), self.backs.clone());
                Box::new(move || {
                    let generation = extractor.arm();
                    answer_back(&backs, Back::ExtractDue { generation });
                })
            },
        };
        tokio::spawn(job.run(plan).in_current_span());
    }
}
