//! 看守查回顾（施工 3-8 四补，`docs/blueprint/kernel/session.md`「回顾」）：
//!
//! - 送进去之前照看守记下的有效历史判：落了盘的里面一条回复都没有的拒绝，`nothing_to_recap`；照到的（落了盘的最后一条人的
//!   消息、回复，替身的组装照这样算）和有效历史里最近一条 `session.recapped` 一样的，交回它；有一次在路上的，并进去，什么
//!   都不出；不然交出一次回顾的请求，照到的就是判出来的那一条，请求照落了盘的日志重建的有效历史组装，一字不差；
//! - 一次只有一个在路上；替身送的回报照内核的规矩算出这一次写没写成（增量对不上、没报发出去、出错、没有正文的都没写成）；
//! - 它的 `model.called` 带 `purpose`、不带回合编号，`seen` 是它照到的那一条，结果和算出来的一样；写成了的后面紧跟着
//!   `session.recapped`（不带回合编号、照到的一样、那一句是替身送的正文去掉前后空白），没写成的没有；
//! - 回应照判出来的：那一句、照到的、是不是交回的，它等的那一条落了盘才回；并进去的一起回；没写成的都拒绝，`recap_failed`；
//! - 对不上在路上的那一次的回报，不理；主对话的几样都不看它（`watch/queue.rs`、`watch/reports.rs`、`watch/compaction.rs`）。

use super::*;
use crate::event::{ModelCalled, Purpose, SessionRecapped};

/// 回顾：在路上的那一次，和等着的回应。
#[derive(Debug, Default)]
pub(in super::super) struct Recaps {
    /// 在路上的那一次。
    pub(in super::super) flight: Option<Flight>,
    /// 等着的回应：哪个命令，它等哪一条落盘，回什么。
    answers: BTreeMap<CommandId, (Seq, Outcome)>,
}

/// 在路上的那一次回顾，和替身送过的回报。
#[derive(Debug)]
pub(in super::super) struct Flight {
    /// 照到第几条。
    pub(in super::super) upto: Seq,
    /// 等着回应的命令，照先后。
    waiting: Vec<CommandId>,
    /// 报过发出去了。
    pub(in super::super) sent: bool,
    /// 正文那一块开始了。
    opened: bool,
    /// 送过的正文。
    text: String,
    /// 增量对不上了。
    broken: bool,
    /// 替身送了说完了：写没写成（写成了的是那一句）。
    ended: Option<Option<String>>,
}

impl Flight {
    /// 说过话了：正文那一块开始了、来过字。
    pub(in super::super) fn said(&self) -> bool {
        !self.text.is_empty()
    }

    /// 下一段像样的增量：正文那一块还没开始的先开始，开始了的来一段字。
    pub(in super::super) fn next_delta(&self) -> Delta {
        match self.opened {
            false => Delta::Start {
                index: 0,
                kind: Kind::Text,
            },
            true => Delta::Text {
                index: 0,
                text: " 做完了一半。".to_string(),
            },
        }
    }
}

/// 送进去之前判出来的。
pub(super) enum Expect {
    /// 拒绝，没有能回顾的。
    Refused(CommandId),
    /// 交回上一句：它是第几条、说的什么、照到的。
    Cached(CommandId, Seq, String, Seq),
    /// 并进在路上的那一次。
    Joined(CommandId),
    /// 交出一次回顾的请求，照到的是这一条。
    Issued(CommandId, Seq),
    /// 在路上的那一次的回报：只记下，不出什么；说完了的追加一批。
    Report,
    /// 对不上在路上的那一次的回报：什么都不出。
    Stale,
}

impl Watch {
    /// 送进一条输入之前：回顾相关的，照规矩判出该怎样；替身送的回报记在在路上的那一次上。
    pub(super) fn before_recap(&mut self, input: &Input, refused: bool) -> Option<Expect> {
        match input {
            Input::Command(received) if received.command == Command::Recap && !refused => {
                Some(self.expect_recap(received.id.clone()))
            }
            Input::AsideSent {
                purpose: Purpose::Recap,
                upto,
                ..
            }
            | Input::AsideDelta {
                purpose: Purpose::Recap,
                upto,
                ..
            }
            | Input::AsideEnded {
                purpose: Purpose::Recap,
                upto,
                ..
            } => {
                let Some(flight) = self.recaps.flight.as_mut().filter(|f| f.upto == *upto) else {
                    self.seen_paths.insert("对不上的回顾回报不理");
                    return Some(Expect::Stale);
                };
                match input {
                    Input::AsideSent { .. } => flight.sent = true,
                    Input::AsideDelta { delta, .. } => flight.take(delta),
                    Input::AsideEnded { error, .. } => {
                        let text = flight.text.trim().to_string();
                        let written = error.is_none() && !flight.broken && flight.sent;
                        flight.ended = Some((written && !text.is_empty()).then_some(text));
                    }
                    _ => {}
                }
                Some(Expect::Report)
            }
            _ => None,
        }
    }

    /// 要一句回顾：照落了盘的有效历史算照到的，和最近一条 `session.recapped` 比，看在不在路上。
    fn expect_recap(&self, id: CommandId) -> Expect {
        let stored = self.pushed.last().copied().unwrap_or(Seq::FIRST);
        let effective = self.effective_events();
        let said: Vec<&Event> = effective
            .iter()
            .filter(|event| event.seq <= stored)
            .filter(|event| matches!(event.body, Body::MessageUser(_) | Body::MessageAssistant(_)))
            .collect();
        if !said
            .iter()
            .any(|event| matches!(event.body, Body::MessageAssistant(_)))
        {
            return Expect::Refused(id);
        }
        let upto = said.last().map_or(Seq::FIRST, |event| event.seq);
        let last = effective.iter().rev().find_map(|event| match &event.body {
            Body::SessionRecapped(recapped) => Some((event.seq, recapped.clone())),
            _ => None,
        });
        if let Some((seq, recapped)) = last
            && recapped.upto == upto
        {
            return Expect::Cached(id, seq, recapped.text, upto);
        }
        match self.recaps.flight {
            Some(_) => Expect::Joined(id),
            None => Expect::Issued(id, upto),
        }
    }

    /// 送进去以后：照判出来的查。
    pub(super) fn after_recap(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        let Some(expect) = expect else {
            return;
        };
        match expect {
            Expect::Refused(id) => {
                self.seen_paths.insert("没有能回顾的被拒");
                assert_eq!(
                    actions,
                    [Action::Reply {
                        id,
                        outcome: Outcome::Rejected {
                            reason: Reason::NothingToRecap
                        },
                    }],
                    "种子 {seed}：没有能回顾的该被拒"
                );
            }
            Expect::Cached(id, seq, text, upto) => {
                self.seen_paths.insert("回顾交回上一句");
                let outcome = Outcome::Recapped {
                    text,
                    upto,
                    cached: true,
                };
                self.recaps
                    .answers
                    .insert(id.clone(), (seq, outcome.clone()));
                let replied: Vec<&Action> = actions.iter().collect();
                assert!(
                    replied.is_empty() || replied == [&Action::Reply { id, outcome }],
                    "种子 {seed}：交回上一句不请求，最多当场回：{actions:?}"
                );
            }
            Expect::Joined(id) => {
                self.seen_paths.insert("回顾并进在路上的");
                assert!(actions.is_empty(), "种子 {seed}：并进去的什么都不出");
                if let Some(flight) = self.recaps.flight.as_mut() {
                    flight.waiting.push(id);
                }
            }
            Expect::Issued(id, upto) => {
                assert!(
                    matches!(actions, [Action::Aside { upto: asked, .. }] if *asked == upto),
                    "种子 {seed}：该交出一次回顾，照到第 {upto} 条：{actions:?}"
                );
                self.recaps.flight = Some(Flight {
                    upto,
                    waiting: vec![id],
                    sent: false,
                    opened: false,
                    text: String::new(),
                    broken: false,
                    ended: None,
                });
            }
            Expect::Report => {
                let ended = self
                    .recaps
                    .flight
                    .as_ref()
                    .is_some_and(|flight| flight.ended.is_some());
                match ended {
                    true => assert!(
                        matches!(actions, [Action::Append(_)]),
                        "种子 {seed}：回顾说完了追加一批，落了盘才回：{actions:?}"
                    ),
                    false => assert!(actions.is_empty(), "种子 {seed}：回顾的回报只记下"),
                }
            }
            Expect::Stale => assert!(actions.is_empty(), "种子 {seed}：对不上的回报不理"),
        }
    }

    /// 交出了回顾的请求：只有一个在路上；请求照落了盘的日志重建的有效历史组装，一字不差。
    pub(super) fn recap_issued(&mut self, upto: Seq, request: &Request) {
        let seed = self.seed;
        self.seen_paths.insert("发了回顾");
        assert!(
            self.recaps
                .flight
                .as_ref()
                .is_some_and(|flight| flight.upto == upto && !flight.sent),
            "种子 {seed}：没判出要回顾就交出了请求"
        );
        let stored = self.pushed.last().copied().unwrap_or(Seq::FIRST);
        let rebuilt = Listing.recap(&self.history_from_log().until(stored));
        assert_eq!(
            rebuilt.map(|(request, upto)| (listed_request(&request), upto)),
            Some((listed_request(request), upto)),
            "种子 {seed}：回顾的请求照落了盘的有效历史组装"
        );
    }

    /// 回顾的 `model.called`：不带回合编号，照到的是在路上的那一次，结果和替身送的算出来的一样；写成了的后面紧跟着
    /// `session.recapped`，没写成的没有。等着的命令照它回。
    pub(super) fn recap_called(&mut self, called: &ModelCalled, events: &[Event], k: usize) {
        let seed = self.seed;
        let flight = self
            .recaps
            .flight
            .take()
            .unwrap_or_else(|| panic!("种子 {seed}：没有在路上的回顾，却记了一条回顾的请求"));
        assert_eq!(events[k].turn, None, "种子 {seed}：回顾不带回合编号");
        assert_eq!(called.purpose, Some(Purpose::Recap), "种子 {seed}");
        assert_eq!(
            called.seen, flight.upto,
            "种子 {seed}：回顾的 seen 是它照到的"
        );
        let written = flight.ended.flatten();
        let after = events.get(k + 1);
        let (seq, outcome) = match written {
            Some(text) => {
                self.seen_paths.insert("回顾写成了");
                assert_eq!(called.result, CallResult::Ok, "种子 {seed}");
                let recapped = after.unwrap_or_else(|| panic!("种子 {seed}：写成了后面是那一句"));
                assert_eq!(
                    recapped.body,
                    Body::SessionRecapped(SessionRecapped {
                        text: text.clone(),
                        upto: flight.upto,
                    }),
                    "种子 {seed}：写成了后面紧跟着那一句"
                );
                let outcome = Outcome::Recapped {
                    text,
                    upto: flight.upto,
                    cached: false,
                };
                (recapped.seq, outcome)
            }
            None => {
                self.seen_paths.insert("回顾没写成");
                assert_eq!(called.result, CallResult::Error, "种子 {seed}");
                assert!(
                    !after.is_some_and(|event| matches!(event.body, Body::SessionRecapped(_))),
                    "种子 {seed}：没写成的没有那一句"
                );
                let outcome = Outcome::Rejected {
                    reason: Reason::RecapFailed,
                };
                (events[k].seq, outcome)
            }
        };
        for id in flight.waiting {
            self.recaps.answers.insert(id, (seq, outcome.clone()));
        }
    }

    /// 一条 `session.recapped`：不带回合编号，前面紧跟着它的回顾的请求（`recap_called` 查过）。
    pub(super) fn recapped_appended(&mut self, event: &Event, events: &[Event], k: usize) {
        let seed = self.seed;
        assert_eq!(event.turn, None, "种子 {seed}：回顾不带回合编号");
        assert_eq!(event.by, By::Kernel, "种子 {seed}");
        assert!(
            k.checked_sub(1).is_some_and(|before| matches!(
                &events[before].body,
                Body::ModelCalled(called) if called.aside()
            )),
            "种子 {seed}：那一句前面紧跟着回顾的请求"
        );
    }

    /// 回顾的回应（写好了的、没写成的）：照判出来的，它等的那一条落了盘。别的回应不看：随机的命令会重发用过的编号，回顾的
    /// 编号也会被当成别的命令再发一次。
    pub(super) fn recap_replied(&mut self, id: &CommandId, outcome: &Outcome) {
        let seed = self.seed;
        if !matches!(
            outcome,
            Outcome::Recapped { .. }
                | Outcome::Rejected {
                    reason: Reason::RecapFailed
                }
        ) {
            return;
        }
        let (seq, expected) = self
            .recaps
            .answers
            .remove(id)
            .unwrap_or_else(|| panic!("种子 {seed}：{id} 的回顾回应没判出来：{outcome:?}"));
        assert_eq!(*outcome, expected, "种子 {seed}：{id} 的回顾回应");
        assert!(
            self.pushed.contains(&seq),
            "种子 {seed}：{id} 的回顾，第 {seq} 条还没推送就回应了"
        );
    }

    /// 最后追加的是一句回顾：刚写好，还没别的事。
    pub(in super::super) fn just_recapped(&self) -> bool {
        self.events
            .last()
            .is_some_and(|event| matches!(event.body, Body::SessionRecapped(_)))
    }

    /// 没有在路上的回顾：崩了、重启了会丢掉它，等着的命令就收不到回应了。
    pub(in super::super) fn recaps_idle(&self) -> bool {
        self.recaps.flight.is_none()
    }
}

impl Flight {
    /// 替身送了一段增量：照内核的累积器算对不对得上。还没报发出去就来的、对不上的，这一次就写不成了。
    fn take(&mut self, delta: &Delta) {
        if self.broken {
            return;
        }
        match delta {
            _ if !self.sent => self.broken = true,
            Delta::Start {
                index: 0,
                kind: Kind::Text,
            } if !self.opened => self.opened = true,
            Delta::Text { index: 0, text } if self.opened => self.text.push_str(text),
            _ => self.broken = true,
        }
    }
}
