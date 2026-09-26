//! 随机一串输入：发消息、重发、空消息、落盘、环境变了、挂接点的结果（对得上的、对不上的、
//! 重复的）随机排。每一步查：
//!
//! - 序号连着；每收到一次命令恰好回应一次，接受的回应都在它的事件推送以后；
//! - 叫跑挂接点时，回合的开头都落了盘；一个回合只叫一次；
//! - 请求模型时，追加过的事件都落了盘，这个回合的挂接点跑完了；一个回合只请求一次；
//!   请求照的是那一刻的全部历史，`seen` 是最后一条。

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::event::ContextInjected;
use crate::id::{FactKind, ModuleId};

/// SplitMix64：随机一串输入用。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// 一路看着会话吐出来的动作，边看边查。
struct Watch {
    seed: u64,
    /// 追加过的事件，照先后。
    events: Vec<Event>,
    pushed: BTreeSet<Seq>,
    /// 每个编号收到了几次、回应了几次。
    received: BTreeMap<CommandId, usize>,
    replied: BTreeMap<CommandId, usize>,
    /// 叫跑过挂接点的回合；送回过对得上的结果的回合；请求过模型的回合。
    hooked: BTreeSet<TurnId>,
    done: BTreeSet<TurnId>,
    called: BTreeSet<TurnId>,
}

impl Watch {
    fn new(seed: u64) -> Watch {
        Watch {
            seed,
            events: Vec::new(),
            pushed: BTreeSet::from([seq(1)]),
            received: BTreeMap::new(),
            replied: BTreeMap::new(),
            hooked: BTreeSet::new(),
            done: BTreeSet::new(),
            called: BTreeSet::new(),
        }
    }

    /// 追加过的最后一条。造会话那一条算在里面。
    fn last(&self) -> u64 {
        self.events.last().map_or(1, |event| event.seq.get())
    }

    /// 送进一条输入之前记下它，送进去以后查吐出来的动作。
    fn feed(&mut self, session: &mut Session, input: Input) {
        match &input {
            Input::Command(command) => {
                *self.received.entry(command.id.clone()).or_default() += 1;
            }
            Input::TurnStartHooksDone { turn, .. } if self.hooked.contains(turn) => {
                self.done.insert(*turn);
            }
            _ => {}
        }
        let actions = session.handle(input);
        for action in actions {
            self.check(action);
        }
    }

    fn check(&mut self, action: Action) {
        let seed = self.seed;
        match action {
            Action::Append(events) => {
                for event in events {
                    assert_eq!(event.seq.get(), self.last() + 1, "种子 {seed}：序号要连着");
                    self.events.push(event);
                }
            }
            Action::Push(events) => self.pushed.extend(events.iter().map(|event| event.seq)),
            Action::Reply { id, outcome } => {
                if let Outcome::Accepted { events } = &outcome {
                    assert!(
                        events.iter().all(|event| self.pushed.contains(event)),
                        "种子 {seed}：{id} 的事件还没推送就回应了"
                    );
                }
                *self.replied.entry(id).or_default() += 1;
            }
            Action::RunTurnStartHooks { turn } => {
                assert!(
                    self.opening(turn).all(|event| self.pushed.contains(&event)),
                    "种子 {seed}：回合 {turn} 的开头还没落盘就跑挂接点"
                );
                assert!(
                    self.hooked.insert(turn),
                    "种子 {seed}：回合 {turn} 叫了两次"
                );
            }
            Action::CallModel { seen, request } => {
                let turn = self.open_turn();
                assert!(
                    self.done.contains(&turn),
                    "种子 {seed}：挂接点还没跑完就请求"
                );
                assert!(
                    self.events
                        .iter()
                        .all(|event| self.pushed.contains(&event.seq)),
                    "种子 {seed}：还有事件没落盘就请求"
                );
                assert_eq!(seen.get(), self.last(), "种子 {seed}：seen 是最后一条");
                let mut all = vec![self.created()];
                all.extend(self.events.iter().cloned());
                assert_eq!(request.system, listing(&all), "种子 {seed}：请求照全部历史");
                assert!(
                    self.called.insert(turn),
                    "种子 {seed}：回合 {turn} 请求了两次"
                );
            }
        }
    }

    /// 回合的开头：`turn.started`，和紧跟着它、同一时刻的内核事实。
    fn opening(&self, turn: TurnId) -> impl Iterator<Item = Seq> + '_ {
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

    /// 正在进行的回合：这一步里只有开，没有结束，所以是最后开的那个。
    fn open_turn(&self) -> TurnId {
        self.events
            .iter()
            .rev()
            .find(|event| matches!(event.body, Body::TurnStarted(_)))
            .map(|event| TurnId::new(event.seq))
            .unwrap_or_else(|| panic!("种子 {}：没开回合就请求", self.seed))
    }

    /// 造会话那一条的样子，替身的组装只看序号和种类。
    fn created(&self) -> Event {
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

/// 挂接点交回来的 0 到 2 块注入。
fn some_injections(rng: &mut Rng) -> Vec<Injection> {
    (0..rng.below(3))
        .map(|k| Injection {
            module: ModuleId::parse(&format!("m{k}")).unwrap(),
            fact: ContextInjected {
                kind: FactKind::parse("memory").unwrap(),
                text: format!("<memory n=\"{k}\"/>"),
            },
        })
        .collect()
}

#[test]
fn random_inputs_keep_the_rules() {
    for seed in 0..300 {
        let mut rng = Rng(seed);
        let mut session = session();
        let mut watch = Watch::new(seed);
        let mut next_id = 1;
        for _ in 0..40 {
            let input = match rng.below(12) {
                0..=3 => {
                    next_id += 1;
                    send(next_id, "hi")
                }
                4 => send(1 + rng.below(next_id), "hi"),
                5 => {
                    next_id += 1;
                    send(next_id, "")
                }
                6 => Input::Environment(environment(if rng.below(2) == 0 { "~/a" } else { "~/b" })),
                7 | 8 => {
                    // 多半是叫过的那个回合，偶尔是对不上的。
                    let turn = match watch.hooked.last() {
                        Some(turn) if rng.below(4) > 0 => *turn,
                        _ => TurnId::new(seq(1 + rng.below(watch.last()))),
                    };
                    Input::TurnStartHooksDone {
                        at: at(30),
                        turn,
                        injected: some_injections(&mut rng),
                    }
                }
                _ => stored(1 + rng.below(watch.last())),
            };
            watch.feed(&mut session, input);
        }
        let last = watch.last();
        watch.feed(&mut session, stored(last));
        assert_eq!(
            watch.received, watch.replied,
            "种子 {seed}：每收到一次命令要恰好回应一次"
        );
    }
}
