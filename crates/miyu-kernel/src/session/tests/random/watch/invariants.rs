//! 看守每一步查的不变量（`docs/designs/02-内核.md` 第九节「不变量怎么查」）。九条里，这里查的是：
//!
//! - 2：回合结束时，它的调用都有了结果（每个调用只有一条结果，在 [`Watch::appended`] 里查）；
//! - 3、6：请求只由日志决定：照日志重建一份有效历史，拿同一个组装器组装，和会话发出去的一字不差；
//! - 4：开回合时没有别的回合开着；
//! - 7：每个编号的回应不多于收到的次数（跑完时收到几次回应几次，在随机测试的末尾查）；
//! - 9：接受过的编号再来，不再产生事件。
//!
//! 1（序号一条接一条）在 [`Watch::appended`] 里查；8（崩了、重启了从日志重建）在载入的那一块查
//! （`watch/load.rs`）；5 这里查不了：随机测试一次只有一个会话。

use super::*;

impl Watch {
    /// 2：回合 `turn` 结束时，它的每个调用都有了结果。
    pub(super) fn all_resulted(&self, turn: TurnId) {
        let seed = self.seed;
        for call in self.calls_in(turn) {
            assert!(
                self.resulted.contains(&call),
                "种子 {seed}：回合 {turn} 结束了，{call} 还没有结果"
            );
        }
    }

    /// 3、6：请求只由日志决定。照看守记下的日志照载入的办法重建有效历史（整份过账本，从还算数的最近一次压缩替代到
    /// 的下一条起留着一切地收、再落到检查点上，施工 6-9），用同一个组装器组装，要和会话发出去的一字不差：载入的那条路
    /// 和活着的那条路走得一样。摘要请求照同一份历史截到 `seen`。
    pub(super) fn request_from_log(&self, seen: Seq, request: &Request) {
        let log: Vec<Event> = std::iter::once(self.created())
            .chain(self.events.iter().cloned())
            .collect();
        let mut ledger = Ledger::default();
        for event in &log {
            ledger
                .append(event)
                .unwrap_or_else(|e| panic!("种子 {}：日志过不了账本：{e}", self.seed));
        }
        let from = ledger.compacted().map_or(Seq::FIRST, Seq::next);
        let mut history = History::whole();
        for event in log.into_iter().filter(|event| event.seq >= from) {
            history.append(event);
        }
        history.settle();
        let rebuilt = match Watch::is_summary(request) {
            true => {
                let instructions = self
                    .manual_turn()
                    .and_then(|manual| manual.instructions.clone());
                Listing.summarize(
                    &history,
                    seen,
                    Watch::truncated(request),
                    instructions.as_deref(),
                )
            }
            false => Listing.assemble(&history),
        };
        assert_eq!(
            listed_request(request),
            listed_request(&rebuilt),
            "种子 {}：会话发出去的请求，和照日志重建的不一样",
            self.seed
        );
    }

    /// 4：开回合时没有别的回合开着。
    pub(super) fn one_turn(&self) {
        assert!(
            !self.turn_open(),
            "种子 {}：还有回合开着，又开了一个",
            self.seed
        );
    }

    /// 7：编号 `id` 的回应不多于收到的次数。
    pub(super) fn replied_at_most_received(&self, id: &CommandId) {
        let received = self.received.get(id).copied().unwrap_or_default();
        let replied = self.replied.get(id).copied().unwrap_or_default();
        assert!(
            replied <= received,
            "种子 {}：{id} 收到 {received} 次，回应了 {replied} 次",
            self.seed
        );
    }

    /// 9：接受过的编号再来，不再产生事件。
    pub(super) fn applied_once(&self, id: &CommandId, actions: &[Action]) {
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, Action::Append(_))),
            "种子 {}：接受过的 {id} 又来了一次，又产生了事件：{actions:?}",
            self.seed
        );
    }
}
