//! 手动压缩（`docs/blueprint/compaction.md` 第七条，施工 6-8）：`session.compact` 空闲时才收，单开一轮只做压缩。
//!
//! 替代到哪在收下命令时就定，和自动压缩同一套（[`Session::upto_before`]，第三条第 2 条）：尾巴一样留（最后一组比预算大的
//! 不留，被动压缩才至少留最后一组）；「这一轮要回应的话」换成还没有哪次请求看到过的人的消息；单开的这一轮自己不压进去。定出来前面没有能压的，拒绝，`nothing_to_compact`。
//!
//! 开的那一轮没有触发，不注入事实、不跑回合开始的挂接点，直接到「准备好」；落了盘发摘要请求，出错再来的照同一个 N、
//! 同样的要求再发。取到了摘要，写 `context.compacted`、同一批结束（[`Session::compacted`]）；失败不数进熔断
//! （`breaker.rs` 的 `after_failure`）。

use super::action::{Action, Reason};
use super::compaction::Due;
use super::turn::{Stage, Turn};
use super::{Session, rejected};
use crate::event::{Body, CompactTrigger, TurnStarted};
use crate::id::{CommandId, Seq, TurnId};
use crate::origin::By;
use crate::request::Request;
use crate::time::Timestamp;

/// 手动压缩单开的那一轮记着的：替代到哪，人附的要求。
#[derive(Debug, Clone)]
pub(super) struct Manual {
    /// 收下命令时定的 N：重试照它，不再算。
    upto: Seq,
    /// 人附的要求，原样；没附的、只有空白的没有。
    instructions: Option<String>,
}

impl Session {
    /// 收下手动压缩的命令：有回合在进行的拒绝，`turn_running`；没有能压的拒绝，`nothing_to_compact`。收下的，单开一轮：
    /// 追加 `turn.started`（没有 `trigger`，`by` 是内核，`cause` 是这个命令），不追加事实，不换实际生效的权限，回合
    /// 直接到「准备好」。它落了盘就回应，附上它的序号；这时发摘要请求（`turn.rs` 的 `ask`）。
    pub(super) fn compact(
        &mut self,
        id: CommandId,
        at: Timestamp,
        instructions: Option<String>,
    ) -> Vec<Action> {
        if self.turn.is_some() {
            return vec![rejected(id, Reason::TurnRunning)];
        }
        let Some(upto) = self.manual_upto() else {
            return vec![rejected(id, Reason::NothingToCompact)];
        };
        let instructions = instructions.filter(|text| !text.trim().is_empty());
        let body = Body::TurnStarted(TurnStarted {
            trigger: None,
            cwd: Some(self.environment.cwd.clone()),
            dirs: self.environment.dirs.clone(),
        });
        let started = self.record(at, By::Kernel, Some(id.clone()), body);
        self.turn = Some(Turn {
            id: TurnId::new(started.seq),
            cause: Some(id.clone()),
            stage: Stage::Ready,
            cwd: self.environment.cwd.clone(),
            dirs: self.environment.dirs.clone(),
            requests: 0,
            retries: 0,
            retrying: false,
            interjected: None,
            queued: Vec::new(),
            refresh: false,
            compacted: false,
            interrupting: None,
            again: None,
            passive: None,
            overflowed: false,
            manual: Some(Manual { upto, instructions }),
        });
        self.accept(id, vec![started.seq]);
        vec![Action::Append(vec![started])]
    }

    /// 手动压缩替代到哪（`compaction.md` 第七条第 2 条）：照还没开的那一轮算，没有触发；尾巴的预算是策略的 `tail`，
    /// 有压缩线的不超过它的四分之一（没报窗口的也能手动压）。策略里没有压缩（6-2 以前造的快照）、没交过限额（算不了
    /// 尾巴）的，没有。
    fn manual_upto(&self) -> Option<Seq> {
        let compaction = self.policy.compaction.as_ref()?;
        let price = self.price()?;
        let budget = self
            .line()
            .map_or(compaction.tail, |line| compaction.tail.min(line / 4));
        self.upto_before(self.ledger.next_seq(), None, budget, &price, false)
    }

    /// 手动压缩那一轮到了「准备好」：照收下命令时定的 N 和要求发摘要请求；压之前的用量照这时组装的 `request` 算。
    /// 不是这一种回合的，没有。
    pub(super) fn manual_due(&self, request: &Request) -> Option<Due> {
        let manual = self.turn.as_ref()?.manual.clone()?;
        Some(Due {
            upto: manual.upto,
            used: self.used(request).unwrap_or(0),
            refills: None,
            trigger: CompactTrigger::Manual,
            instructions: manual.instructions,
        })
    }
}
