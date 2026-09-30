//! 她正忙时跟住听到这一句的那一轮（施工 7-10，`docs/blueprint/cli/ask.md`「怎么走」第 8 条，2026-09-30 主会话定）。
//!
//! 她正忙，这一句不另开一轮：人的话排进在进行的那一轮（带着回合编号），别的 harness 的话照回报的规矩到（不带），都由在进行
//! 的那一轮下一步听到。所以推过来的那条 `message.user`（`cause` 是自己的命令）落了盘以后，带回合编号的推送是哪一轮的就
//! 接上哪一轮：它自己带着的当场接上，不带的接它后面第一条。闲着时由它开的那一轮照 `turn.started` 认（`waiting.rs`）。
//!
//! 接上的那一轮结束时它的请求一次都没看到这一句（每次的 `seen` 都比这一句的序号小：她在最后一步，或者被打断了），不收尾：
//! 内核同一批接着开下一轮（打断时排着的接着发的也是），那一轮才听到它。打断时退回了的、别的 harness 的话被打断以后只记下
//! 的，没有下一轮马上来，这边一直等，靠 Ctrl+C、`--timeout` 出来（`cli/ask.md`「还没有的」）。

use serde_json::{Value, json};

use super::{Follow, Screen};

/// 这一句落了盘没有、跟着的这一轮是不是接上的、它听到这一句没有。
#[derive(Debug, Default)]
pub(super) struct Joining {
    /// 这一句落了盘：推过来的那条 `message.user` 的序号。
    landed: Option<u64>,
    /// 跟着的这一轮是接上的，不是这一句开的。
    joined: bool,
    /// 跟着的这一轮的请求看到了第几条为止：`model.called` 的 `seen`，取最大的。
    seen: u64,
}

impl Joining {
    /// 开始跟一轮：`joined` 是它是接上的。
    pub(super) fn begin(&mut self, joined: bool) {
        self.joined = joined;
        self.seen = 0;
    }

    /// 一次请求的记录（`model.called` 的 `body`）：看到了第几条为止。
    pub(super) fn called(&mut self, body: &Value) {
        self.seen = self.seen.max(body["seen"].as_u64().unwrap_or(0));
    }

    /// 跟着的这一轮结束了：是接上的、没听到这一句，下一轮才是，交回真。
    pub(super) fn missed(&mut self) -> bool {
        let (joined, seen) = (self.joined, self.seen);
        self.begin(false);
        joined && self.landed.is_some_and(|landed| seen < landed)
    }
}

impl Follow<'_> {
    /// 第一轮还没认出来时，每一条推送的事件 `event` 先过这里（`turn.started` 不过这里，另由 `started` 认）：是这一句落了盘
    /// 的，记下序号；这一句落了盘以后，带着回合编号的是在进行的那一轮的，接上它。这一句自己带着回合编号的（人的话排进了
    /// 那一轮），当场就接上。
    pub(super) fn join(&mut self, event: &Value, screen: &mut Screen<'_>) {
        if self.turn.is_some() || self.ended_first {
            return;
        }
        if self.joining.landed.is_none() {
            if event["kind"] != "message.user" || event["cause"] != json!(self.sent) {
                return;
            }
            self.joining.landed = event["seq"].as_u64();
        }
        if let Some(turn) = event["turn"].as_u64() {
            self.begin(Some(turn), true, screen);
        }
    }
}
