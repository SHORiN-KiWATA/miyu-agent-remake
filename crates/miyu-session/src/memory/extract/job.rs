//! 派出去抽的那一件活（施工 R-6 上，`docs/blueprint/memory.md` 第六条第 6 到 8 款；施工 R-7 补从 `extract.rs` 挪出来）：照
//! 定好的办法跳过、发、读、遮、记，记下以后看一眼够不够合并。

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::oneshot;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{AccountId, Seq, SessionId};
use miyu_kernel::request::Message;
use miyu_kernel::time::UtcOffset;
use miyu_recall::Skipped;
use miyu_recall::extract::candidates;
use miyu_recall::redact::redact;
use miyu_tool::TEXT_CHARS;

use super::{Extraction, Extractor, PURPOSE, Plan, TRIES, secrets};
use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::config::TurnConfig;
use crate::memory::merge::MergeJob;
use crate::memory::{Dreamed, Keeper, NotDreamed};
use crate::route::Ask;

/// 派出去抽的一次。
pub(crate) struct Job {
    /// 记在哪一间、听众是谁。
    pub(crate) keeper: Keeper,
    /// 字、key 的写法、一次性入口。
    pub(crate) extraction: Extraction,
    /// 这一轮的配置：照它取 key、发。
    pub(crate) config: TurnConfig,
    /// 记在谁的账上：会话的属主。
    pub(crate) owner: AccountId,
    /// 哪个会话。
    pub(crate) session: SessionId,
    /// 从第几条以后抽（上次抽到的；没抽过的是 0 那种，照 `Seq` 最小的）。
    pub(crate) after: Seq,
    /// 这一回的状态：走完了清掉。
    pub(crate) extractor: Arc<Extractor>,
    /// 放不下、还剩几轮的，抽完了照它马上再起一次（不用再等闲）。
    pub(crate) again: Box<dyn Fn() + Send + Sync>,
    /// 会话的时区：合并的日期照它写（施工 R-7 上）。
    pub(crate) offset: UtcOffset,
    /// 人叫她现在就整理（施工 R-7 补，`/dream`）：抽完了不看够不够、现在就合，合完把几样数交回这一头。没叫的照定时的看一眼。
    pub(crate) dream: Option<oneshot::Sender<Result<Dreamed, NotDreamed>>>,
}

impl Job {
    /// 照 `plan` 走一次：跳过的只记抽到了哪；要发的发、读、遮、记。之后：人叫了现在就整理的现在就合、把结果交回去；没叫的
    /// 记下了抽到哪就看一眼够不够合并。
    pub(crate) async fn run(mut self, plan: Plan) {
        let recorded = match plan {
            Plan::Wait => {
                self.extractor.finish(self.after, true);
                false
            }
            Plan::Skip { upto } => {
                let marked = self
                    .mark(upto, Vec::new(), Some(Skipped::Remembered))
                    .await
                    .is_some();
                self.extractor.finish(self.after, true);
                marked
            }
            Plan::Ask {
                upto,
                text,
                turns,
                more,
            } => {
                let recorded = self.ask(upto, text, turns).await;
                if recorded && more {
                    (self.again)();
                }
                recorded
            }
        };
        match self.dream.take() {
            Some(reply) => {
                let dreamed = self.merge_job().now().await;
                if reply.send(dreamed).is_err() {
                    tracing::debug!(target: TARGET, session = %self.session, "dream answer not taken");
                }
            }
            None if recorded => self.merge_job().start(),
            None => {}
        }
    }

    /// 发、读、遮、记；交回记成了没有。
    async fn ask(&self, upto: Seq, text: String, turns: BTreeSet<u64>) -> bool {
        let started = Instant::now();
        let secrets = secrets(&self.config);
        let shapes = self.extraction.shapes.clone();
        let organizer =
            crate::settings::MemorySettings::from(&self.config.resolved.values()).organizer;
        tracing::info!(target: TARGET, session = %self.session, turns = turns.len(), "memory extraction started");
        let ask = Ask {
            model: organizer,
            purpose: PURPOSE.to_string(),
            system: String::new(),
            messages: vec![Message::User {
                blocks: vec![Block::Text(Text {
                    text: redact(&text, &secrets, &shapes),
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
            .and_then(|answer| candidates(&answer.text, &turns, TEXT_CHARS));
        match answered {
            Ok(mut found) => {
                for candidate in &mut found {
                    candidate.text = redact(&candidate.text, &secrets, &shapes);
                }
                let recorded = self.mark(upto, found, None).await;
                if let Some(count) = recorded {
                    let took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    tracing::info!(target: TARGET, session = %self.session, count, took_ms, "memory extracted");
                    if count > 0 {
                        self.fill().await;
                    }
                }
                self.extractor.finish(self.after, true);
                recorded.is_some()
            }
            Err(why) => {
                let tries = self.extractor.finish(self.after, false);
                tracing::warn!(target: TARGET, session = %self.session, tries, error = why.as_str(), "memory extraction failed");
                if tries >= TRIES {
                    self.mark(upto, Vec::new(), Some(Skipped::Failed)).await;
                }
                false
            }
        }
    }

    /// 记下了几条以后在后台补它们的向量（施工 R-5 五补：不等下一次搜）。
    async fn fill(&self) {
        let keeper = self.keeper.clone();
        let using = crate::memory::Using {
            config: Arc::clone(&self.config),
            owner: self.owner.clone(),
        };
        blocking(move || keeper.fill(&using)).await;
    }

    /// 照这一次的几样合并（施工 R-7 上，`merge.rs`）：定时的看一眼够不够、在后台合，人叫的现在就合。
    fn merge_job(&self) -> MergeJob {
        MergeJob {
            keeper: self.keeper.clone(),
            extraction: self.extraction.clone(),
            config: Arc::clone(&self.config),
            owner: self.owner.clone(),
            offset: self.offset,
        }
    }

    /// 记下几条、记抽到了第 `upto` 条，交回记下几条；写不进的记一行，交回没有。
    async fn mark(
        &self,
        upto: Seq,
        found: Vec<miyu_recall::extract::Candidate>,
        skipped: Option<Skipped>,
    ) -> Option<u32> {
        let (keeper, session) = (self.keeper.clone(), self.session.clone());
        let recorded =
            blocking(move || keeper.record_extraction(wall_now(), &session, upto, found, skipped))
                .await;
        recorded
            .inspect_err(|error| {
                tracing::warn!(target: TARGET, session = %self.session, error = ?error, "memory extraction not recorded");
            })
            .ok()
    }
}
