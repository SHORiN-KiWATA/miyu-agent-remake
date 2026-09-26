//! 出错以后再来（`docs/designs/02-内核.md` 第六节「回复怎么收」第 4 条，施工 3-5 下）。
//!
//! 可以重试的几类（可重试、限速、增量对不上、回复里一个块都没有），这一步连着不到 5 次，就等一会儿
//! 再来：什么都没收到的，再组装一次就是原样的请求（`model.called` 不进上下文）；收到了一半的，半截
//! 已经写成回复（`call.rs`），后面追加一条被打断的事实，她看到自己说了一半，接着往下说。
//!
//! 等多久：供应商说了的照它，最多 2 分钟，说得更久的不等；没说的 1、2、4、8、16 秒。内核是纯逻辑，
//! 自己不睡，交出「到点叫醒」；等的时候推一条 `status`。重试不算步数。

use super::Session;
use super::action::Action;
use super::turn::Stage;
use crate::event::{Body, CallError, ErrorClass, Event, Retry, Status, Transient, TransientBody};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

/// 一步里最多重试几次（`15-模型与供应商.md` M5，2026-09-26 项目主人定）。
pub(super) const RETRY_LIMIT: u32 = 5;

/// 供应商说要等的，最多等多久：更久的不等，这一轮以出错结束。
const WAIT_LIMIT_MS: u64 = 120_000;

impl Session {
    /// 这个错要不要再来、等多久：可以重试的几类，这一步连着还没到上限，要等的不超过 2 分钟。
    pub(super) fn retry_wait(&self, error: &CallError, wait_ms: Option<u64>) -> Option<u64> {
        let turn = self.turn.as_ref()?;
        if !retryable(&error.class) || turn.retries >= RETRY_LIMIT {
            return None;
        }
        let wait = wait_ms.unwrap_or_else(|| backoff(turn.retries + 1));
        (wait <= WAIT_LIMIT_MS).then_some(wait)
    }

    /// 等着再来：收到了一半的（`cut`），半截已经写成回复，后面追加被打断的那一句；回合停在
    /// 「等着重试」，推一条 `status`，交出到点叫醒。
    #[expect(
        clippy::too_many_arguments,
        reason = "一次出错收场要的都在这里：时刻、哪次请求、cause、这一批事件、有没有半截、出了什么错、等多久"
    )]
    pub(super) fn wait_to_retry(
        &mut self,
        at: Timestamp,
        seen: Seq,
        cause: Option<CommandId>,
        mut events: Vec<Event>,
        cut: bool,
        error: CallError,
        wait: u64,
    ) -> Vec<Action> {
        if cut {
            let fact = self.policy.facts.reply_cut();
            events.push(self.record(at, By::Kernel, cause.clone(), Body::ContextInjected(fact)));
        }
        let Some(turn) = self.turn.as_mut() else {
            return vec![Action::Append(events)];
        };
        turn.retries += 1;
        turn.retrying = true;
        turn.stage = Stage::Waiting { after: seen };
        let status = Transient {
            at,
            turn: Some(turn.id),
            by: By::Kernel,
            cause,
            body: TransientBody::Status(Status {
                seen,
                retry: Retry {
                    attempt: turn.retries,
                    limit: RETRY_LIMIT,
                    wait_ms: wait,
                    class: error.class,
                    message: error.message,
                },
            }),
        };
        vec![
            Action::Append(events),
            Action::PushTransient(status),
            Action::Wake {
                at: later(at, wait),
                seen,
            },
        ]
    }

    /// 到点了：在等的正是这一次，回到「准备好」；等的时候切过级别的，先把事实查一遍（和往下走的
    /// 请求一样），再照有效历史组装一次。别的都不理：被打断了、结束了、已经不在等了。
    pub(super) fn woke(&mut self, at: Timestamp, seen: Seq) -> Vec<Action> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        if !matches!(turn.stage, Stage::Waiting { after } if after == seen) {
            return Vec::new();
        }
        turn.stage = Stage::Ready;
        let events = self.refresh_facts(at);
        let mut actions = Vec::new();
        if !events.is_empty() {
            actions.push(Action::Append(events));
        }
        actions.extend(self.advance());
        actions
    }
}

/// 可以重试的几类。上下文超长的先压缩（M6），认证失败、内容策略、其他，重试也没用。
fn retryable(class: &ErrorClass) -> bool {
    matches!(
        class,
        ErrorClass::Retryable
            | ErrorClass::RateLimited
            | ErrorClass::BadStream
            | ErrorClass::EmptyReply
    )
}

/// 供应商没说等多久的，第几次重试就等 1、2、4、8、16 秒。
fn backoff(attempt: u32) -> u64 {
    1000 << attempt.saturating_sub(1).min(10)
}

/// `at` 再过 `wait` 毫秒。超出了能写的范围的，就是 `at`：到了就马上叫醒。
fn later(at: Timestamp, wait: u64) -> Timestamp {
    i64::try_from(wait)
        .ok()
        .and_then(|wait| at.unix_millis().checked_add(wait))
        .and_then(Timestamp::from_unix_millis)
        .unwrap_or(at)
}
