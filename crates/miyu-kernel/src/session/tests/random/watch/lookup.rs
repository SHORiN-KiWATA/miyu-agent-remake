//! 看守翻日志的几样：回合开着没有、回合的开头、正在进行的回合、一条回复里的调用、
//! 一个回合里的调用、调用的是哪件工具、造会话那一条的样子和开始时的权限。

use super::*;

impl Watch {
    /// 有没有回合开着：最后开的那个还没结束。
    pub(super) fn turn_open(&self) -> bool {
        let started = self
            .events
            .iter()
            .rposition(|event| matches!(event.body, Body::TurnStarted(_)));
        let ended = self
            .events
            .iter()
            .rposition(|event| matches!(event.body, Body::TurnEnded(_)));
        match (started, ended) {
            (Some(started), Some(ended)) => started > ended,
            (Some(_), None) => true,
            _ => false,
        }
    }

    /// 回合的开头：`turn.started`，和紧跟着它、同一时刻的内核事实。
    pub(super) fn opening(&self, turn: TurnId) -> impl Iterator<Item = Seq> + '_ {
        let start = self
            .events
            .iter()
            .position(|event| event.seq == turn.started())
            .unwrap_or_else(|| panic!("种子 {}：回合 {turn} 没开过", self.seed));
        let at = self.events[start].at;
        self.events[start..]
            .iter()
            .take_while(move |event| event.at == at && event.by == By::Kernel)
            .map(|event| event.seq)
    }

    /// 正在进行的回合：一个接一个，所以是最后开的那个。
    pub(super) fn open_turn(&self) -> TurnId {
        self.events
            .iter()
            .rev()
            .find(|event| matches!(event.body, Body::TurnStarted(_)))
            .map(|event| TurnId::new(event.seq))
            .unwrap_or_else(|| panic!("种子 {}：还没开过回合", self.seed))
    }

    /// 第 `reply` 号回复里的调用。
    pub(super) fn calls_of(&self, reply: Seq) -> impl Iterator<Item = CallId> + '_ {
        self.events
            .iter()
            .filter(move |event| event.seq == reply)
            .flat_map(|event| match &event.body {
                Body::MessageAssistant(reply) => reply.blocks.clone(),
                _ => Vec::new(),
            })
            .filter_map(|block| match block {
                Block::ToolCall(call) => Some(call.call_id),
                _ => None,
            })
    }

    /// 回合 `turn` 里全部回复的调用。
    pub(super) fn calls_in(&self, turn: TurnId) -> impl Iterator<Item = CallId> + '_ {
        self.events
            .iter()
            .filter(move |event| event.turn == Some(turn))
            .filter(|event| matches!(event.body, Body::MessageAssistant(_)))
            .flat_map(|event| self.calls_of(event.seq).collect::<Vec<_>>())
    }

    /// 调用 `call_id` 调的是哪件工具。
    pub(super) fn name_of(&self, call_id: CallId) -> String {
        self.events
            .iter()
            .filter(|event| event.seq == call_id.message())
            .find_map(|event| match &event.body {
                Body::MessageAssistant(reply) => {
                    reply.blocks.iter().find_map(|block| match block {
                        Block::ToolCall(call) if call.call_id == call_id => Some(call.name.clone()),
                        _ => None,
                    })
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("种子 {}：没有调用 {call_id}", self.seed))
    }

    /// 造会话那一条的样子，替身的组装只看序号和种类。
    pub(super) fn created(&self) -> Event {
        let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
        Event {
            seq: seq(1),
            at: at(0),
            turn: None,
            by: alice(),
            cause: Some(id(0)),
            body: Body::SessionCreated(created),
        }
    }
}

/// 造会话时的权限。
pub(super) fn created_permission() -> Permission {
    let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    created.permission
}
