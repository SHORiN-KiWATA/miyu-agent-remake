//! 一轮开始、结束（蓝图 `tui.md`「正文」「时间线」）：开轮时把排队的消息挪进正文、记下这一轮；结束时停表、
//! 收起时间线、写收尾行，没收完的块算收全。

use super::*;

impl Transcript {
    /// 在等她的第一个字：一轮在跑、这一轮还一块都没来（出错等重试也算）。界面在正文末尾转圈，
    /// 步与步之间等她不算（蓝图 `tui.md`「时间线」第 19 条）；正在压缩时那一行自己在转，不算（「正文」第 9 条）。
    pub fn waiting(&self) -> bool {
        self.running.is_some() && !self.spoke && self.compacting.is_none()
    }

    /// 她正在写的这一段正文：这一轮在跑，正文最后一条是这一轮的回答。长过视口时正文区停在它的开头
    /// （蓝图 `tui.md`「正文」第 1 条）。
    pub fn writing(&self) -> Option<&Entry> {
        self.running?;
        let last = &self.entries[self.tail()?];
        (last.kind == Kind::Reply && last.turn == self.turn).then_some(last)
    }

    /// 最新的那段回答是第几条、是不是还在写（蓝图 `tui.md`「正文」第 1 条，长过视口时停在它的开头）：在写的那一段；一轮
    /// 结束了的，这一轮的最后一段回答、后面只有收尾那一行（写完以后才画出来的图、链接卡片会把它撑高）。
    pub fn reading(&self) -> Option<(usize, bool)> {
        let tail = self.tail()?;
        let last = &self.entries[tail];
        if self.running.is_some() {
            return (last.kind == Kind::Reply && last.turn == self.turn).then_some((tail, true));
        }
        let reply = tail.checked_sub(1)?;
        let before = &self.entries[reply];
        (last.kind == Kind::Done && before.kind == Kind::Reply && before.turn == last.turn)
            .then_some((reply, false))
    }

    /// 一轮开始了（`turn.started`）：开表、清这一轮的用量，把开这一轮的排队消息挪进正文。
    pub(super) fn start(&mut self, turn: u64, trigger: Option<u64>) {
        self.running = Some(self.clock.now());
        self.spoke = false;
        self.opened_by.clear();
        self.failure = None;
        self.turn = Some(turn);
        self.turn_usage = Usage::default();
        self.turn_level = self.level;
        // 手动压缩那一轮没有 `trigger`，不是哪一句开的（施工 6-8，蓝图「正文」第 9 条）。
        self.manual = trigger.is_none();
        self.cleared = false;
        if trigger.is_none() {
            return;
        }
        // 开这一轮的那一句：照 `trigger` 找序号对得上的；序号还没配上的，退回找还没归到哪一轮的最早那一句。
        let by_seq = self
            .entries
            .iter()
            .position(|e| trigger.is_some() && e.seq == trigger);
        let oldest = || {
            self.entries
                .iter()
                .position(|e| e.kind == Kind::User && e.turn.is_none())
        };
        let Some(at) = by_seq.or_else(oldest) else {
            return;
        };
        // 它和排在它前面、还排着的几条一起开这一轮：两下 Esc 打断以后排着的一起发，`trigger` 只指着其中一条
        // （`tui.md`「运行状态行和排队的消息」第 5 条）。排着的挪到正文末尾，按先后；本来不是排着的留在原位。
        let upto = self.entries[at].seq;
        let joins = |j: usize, e: &Entry| {
            j == at
                || (e.queued
                    && e.turn.is_none()
                    && match (e.seq, upto) {
                        (Some(a), Some(b)) => a <= b,
                        _ => j < at,
                    })
        };
        let picked: Vec<usize> = (0..self.entries.len())
            .filter(|&j| joins(j, &self.entries[j]))
            .collect();
        self.bring_in(&picked, turn);
    }

    /// 一轮结束了（`turn.ended`）：停表、收起时间线、写收尾行或打断、出错的那一行。没有在进行的不管：核心断开时
    /// 界面当场收过尾了，核心重启以后补推的那一轮的结束不再写（「连核心」第 7 条）。
    pub(super) fn end(&mut self, reason: EndReason, texts: &Texts) {
        if self.turn.is_none() && self.running.is_none() {
            return;
        }
        let (took, failure) = self.wind_up();
        match reason {
            // 清空那一轮：正文已经有「上下文已清空」那一行，不另起收尾行、不接用时（「正文」第 9 条）。
            EndReason::Completed if self.cleared => {}
            EndReason::Completed => {
                // 手动压缩压好了：用时和用量接在结果那一行后面，不另起收尾行（「正文」第 9 条）。
                let usage = words::turn_usage(&self.turn_usage, texts);
                let tail = format!(" · {}{usage}", crate::meter::seconds(took));
                if self.manual && self.append_to_result(&tail) {
                    self.turn = None;
                    return;
                }
                let (model, endpoint) = self
                    .model
                    .as_ref()
                    .map_or(("", ""), |(m, e)| (m.as_str(), e.as_str()));
                let time = self.clock.wall().strftime("%H:%M").to_string();
                let text = texts
                    .done
                    .replace("{endpoint}", endpoint)
                    .replace("{model}", model)
                    .replace("{elapsed}", &crate::meter::seconds(took))
                    .replace("{time}", &time);
                self.push(Kind::Done, text + &usage);
            }
            // 和收尾行一个样子：`✻ 已中断`（`tui.md`「正文」第 4 条）。
            EndReason::Interrupted => self.push(Kind::Done, texts.interrupted.clone()),
            EndReason::Error => {
                let reason = failure.map_or_else(
                    || super::failure::class_name("other", texts),
                    |error| super::failure::reason(&error, texts),
                );
                let text = texts.failed.replace("{reason}", &reason);
                self.push(Kind::Error, text);
            }
            EndReason::Other(other) => self.push(Kind::Note, other),
        }
        self.turn = None;
    }

    /// 核心断开了：在进行的那一轮当场收尾，写一行和被打断的一个样子、整行红的（「连核心」第 7 条）。
    pub(super) fn cut_off(&mut self, texts: &Texts) {
        if self.turn.is_none() && self.running.is_none() {
            return;
        }
        self.wind_up();
        self.push(Kind::Cut, texts.turn_cut.clone());
        self.turn = None;
        // 还排着、没被请求带上的话：已经在核心的日志里，下一次请求会带上，接在断开那一行后面进正文，不再悬着
        // （2026-10-02 项目主人报：它看不见、等下一句开轮时被挪到那一句后面）。
        self.unqueue_to_end();
    }

    /// 一轮到头了：停表、收起时间线、停掉还在转的步。交回用了多久、记着的出错。
    fn wind_up(&mut self) -> (std::time::Duration, Option<crate::core::CallError>) {
        let now = self.clock.now();
        let took = self
            .running
            .take()
            .map_or(std::time::Duration::ZERO, |start| {
                now.saturating_duration_since(start)
            });
        self.retry = None;
        self.drop_compacting();
        self.blocks.clear();
        self.unnamed.clear();
        self.finish_segment();
        self.settle_leftovers();
        (took, self.failure.take())
    }

    /// 一次工具调用的参数（收齐以后读成的值）：确认的抽屉照它写命令的短标题、原文（「确认和提问的抽屉」第 3 条）。
    pub fn tool_args(&self, call_id: &str) -> Option<&serde_json::Value> {
        let segments = self.entries.iter().filter_map(|e| e.segment.as_ref());
        segments
            .flat_map(|s| s.steps.iter())
            .find(|step| step.call_id.as_deref() == Some(call_id))
            .and_then(|step| match &step.kind {
                StepKind::Tool { parsed, .. } => Some(parsed),
                StepKind::Thought { .. } => None,
            })
    }

    /// 这一轮结束了还没有结果的步骤（被打断的）：停表、不再转圈。
    pub(super) fn settle_leftovers(&mut self) {
        let now = self.clock.now();
        let segments = self.entries.iter_mut().filter_map(|e| e.segment.as_mut());
        for step in segments.flat_map(|s| s.steps.iter_mut()) {
            if step.busy() {
                step.stop_at(now);
                if let StepKind::Tool { state, .. } = &mut step.kind {
                    *state = ToolState::Done(ToolStatus::Cancelled);
                }
            }
        }
    }

    /// `order` 里的下一档，到头回到第一档；现在的不在里面的，是第一档。只算不改：切到哪一档由核心推来的
    /// `session.policy_changed` 定（蓝图 `tui.md`「权限级别」第 2 条）。
    pub fn next_level(&self, order: &[Level]) -> Level {
        let at = order.iter().position(|l| *l == self.level);
        let next = at.map_or(0, |i| (i + 1) % order.len().max(1));
        order.get(next).copied().unwrap_or(self.level)
    }
    /// 在跑的（刚结束的）这一轮是不是手动压缩、清空：核心单开的一轮，不是回答（「系统通知」第 1 条）。
    pub fn manual_turn(&self) -> bool {
        self.manual
    }
}
