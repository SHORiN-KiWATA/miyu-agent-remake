//! 随机一串输入：发消息、重发、空消息、落盘、环境变了、挂接点的结果、执行器的三种回报，
//! 对得上的、对不上的、先后乱了的，随机排。每一步查：
//!
//! - 序号连着；每收到一次命令恰好回应一次，接受的回应都在它的事件推送以后；
//! - 叫跑挂接点时，回合的开头都落了盘；一个回合只叫一次；
//! - 请求模型时，追加过的事件都落了盘，这个回合的挂接点跑完了；一个回合只请求一次；
//!   请求照的是那一刻的全部历史，`seen` 是最后一条；
//! - 每次请求至多一条 `model.called`：正常说完的排在它的回复后面，出错的后面紧跟着出错的
//!   `turn.ended`；推给头的增量是在路上的那次请求的；叫执行器别再发的，这次请求已经记了出错；
//! - 回合结束的挂接点，等 `turn.ended` 落了盘才跑，一个回合一次。

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::{CallError, CallResult, ErrorClass, TransientBody};
use crate::id::{ContentHash, FactKind, ModelName, ModuleId, ProviderId};
use crate::origin::Model;

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
    /// 叫跑过挂接点的回合；送回过对得上的结果的回合；请求过模型的回合；跑过结束挂接点的回合。
    hooked: BTreeSet<TurnId>,
    done: BTreeSet<TurnId>,
    called: BTreeSet<TurnId>,
    end_hooked: BTreeSet<TurnId>,
    /// 交给执行器的请求；报过「发出去了」的；记了 `model.called` 的。
    issued: BTreeSet<Seq>,
    sent: BTreeSet<Seq>,
    recorded: BTreeSet<Seq>,
    /// 在路上的那次请求。
    asking: Option<Seq>,
    /// 走到过哪些路：随机的输入要真走到这些地方，查的才不是空话。
    seen_paths: BTreeSet<&'static str>,
    /// 在路上的那次请求，下一块从第几块开始、哪一块还开着：执行器替身多半照这个送像样的增量。
    next_block: usize,
    open_block: Option<usize>,
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
            end_hooked: BTreeSet::new(),
            issued: BTreeSet::new(),
            sent: BTreeSet::new(),
            recorded: BTreeSet::new(),
            asking: None,
            seen_paths: BTreeSet::new(),
            next_block: 0,
            open_block: None,
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
            Input::RequestSent { seen, .. } if Some(*seen) == self.asking => {
                self.sent.insert(*seen);
            }
            _ => {}
        }
        for action in session.handle(input) {
            self.check(action);
        }
    }

    fn check(&mut self, action: Action) {
        let seed = self.seed;
        match action {
            Action::Append(events) => self.appended(events),
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
                assert_eq!(
                    listed_request(&request),
                    listing(&all),
                    "种子 {seed}：请求照全部历史"
                );
                assert!(
                    self.called.insert(turn),
                    "种子 {seed}：回合 {turn} 请求了两次"
                );
                self.issued.insert(seen);
                self.asking = Some(seen);
                self.next_block = 0;
                self.open_block = None;
            }
            Action::PushTransient(transient) => {
                self.seen_paths.insert("推了增量");
                let TransientBody::ModelDelta(delta) = &transient.body;
                assert_eq!(
                    Some(delta.seen),
                    self.asking,
                    "种子 {seed}：推的增量不是在路上的那次请求的"
                );
                assert!(
                    self.sent.contains(&delta.seen),
                    "种子 {seed}：请求还没发出去就推了增量"
                );
                assert!(matches!(transient.by, By::Model(_)));
                assert_eq!(transient.turn, Some(self.open_turn()));
            }
            Action::CancelModel { seen } => {
                self.seen_paths.insert("叫执行器别再发");
                assert!(
                    self.recorded.contains(&seen),
                    "种子 {seed}：叫执行器别再发的请求 {seen}，没记出错"
                );
            }
            Action::RunTurnEndHooks { turn } => {
                self.seen_paths.insert("跑了回合结束的挂接点");
                let ended = self.events.iter().find(|event| {
                    event.turn == Some(turn) && matches!(event.body, Body::TurnEnded(_))
                });
                assert!(
                    ended.is_some_and(|event| self.pushed.contains(&event.seq)),
                    "种子 {seed}：回合 {turn} 的结束还没落盘就跑挂接点"
                );
                assert!(
                    self.end_hooked.insert(turn),
                    "种子 {seed}：回合 {turn} 的结束挂接点跑了两次"
                );
            }
        }
    }

    /// 追加的一批：序号连着；`model.called` 每次请求至多一条，排在它的回复后面，出错的后面
    /// 紧跟着出错的 `turn.ended`。
    fn appended(&mut self, events: Vec<Event>) {
        let seed = self.seed;
        for (k, event) in events.iter().enumerate() {
            assert_eq!(event.seq.get(), self.last() + 1, "种子 {seed}：序号要连着");
            match &event.body {
                Body::TurnStarted(_) if !self.hooked.is_empty() => {
                    self.seen_paths.insert("开了第二轮");
                }
                Body::MessageAssistant(reply)
                    if reply
                        .blocks
                        .iter()
                        .any(|block| matches!(block, Block::ToolCall(_))) =>
                {
                    self.seen_paths.insert("回复里有工具调用");
                }
                _ => {}
            }
            if let Body::ModelCalled(called) = &event.body {
                assert!(
                    self.issued.contains(&called.seen),
                    "种子 {seed}：没交给执行器的请求 {} 记了一条",
                    called.seen
                );
                assert!(
                    self.recorded.insert(called.seen),
                    "种子 {seed}：请求 {} 记了两条",
                    called.seen
                );
                let before = k.checked_sub(1).map(|k| &events[k].body);
                let after = events.get(k + 1).map(|event| &event.body);
                if called.result == CallResult::Ok {
                    self.seen_paths.insert("说完了");
                    assert!(
                        matches!(before, Some(Body::MessageAssistant(reply)) if reply.seen == called.seen),
                        "种子 {seed}：说完了的，前面是它的回复"
                    );
                } else {
                    self.seen_paths.insert("出错了");
                    assert!(
                        matches!(after, Some(Body::TurnEnded(ended)) if ended.reason == crate::event::EndReason::Error),
                        "种子 {seed}：出错的，后面紧跟着出错的回合结束"
                    );
                }
                if self.asking == Some(called.seen) {
                    self.asking = None;
                }
            }
            self.events.push(event.clone());
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

    /// 正在进行的回合：一个接一个，所以是最后开的那个。
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

    /// 下一段增量：多半接着在路上的那次请求像样地往下说，偶尔乱来。
    fn some_delta(&mut self, rng: &mut Rng) -> Delta {
        if rng.below(6) == 0 {
            return scrambled_delta(rng);
        }
        match self.open_block {
            None => {
                let index = self.next_block;
                self.next_block += 1;
                self.open_block = Some(index);
                let kind = if rng.below(3) == 0 {
                    Kind::ToolCall {
                        name: "read".to_string(),
                    }
                } else {
                    Kind::Text
                };
                Delta::Start { index, kind }
            }
            Some(index) if rng.below(3) == 0 => {
                self.open_block = None;
                Delta::End { index }
            }
            Some(index) => Delta::Text {
                index,
                text: "x".to_string(),
            },
        }
    }

    /// 多半是在路上的那次请求，偶尔是对不上的。
    fn some_seen(&self, rng: &mut Rng) -> Seq {
        match self.asking {
            Some(seen) if rng.below(5) > 0 => seen,
            _ => seq(1 + rng.below(self.last())),
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

/// 乱来的一段增量：头两块里的一块，正文或者工具调用；先后乱了的，累积器会报错。
fn scrambled_delta(rng: &mut Rng) -> Delta {
    let index = rng.below(2) as usize;
    match rng.below(4) {
        0 => Delta::Start {
            index,
            kind: if rng.below(3) == 0 {
                Kind::ToolCall {
                    name: "read".to_string(),
                }
            } else {
                Kind::Text
            },
        },
        1 | 2 => Delta::Text {
            index,
            text: "x".to_string(),
        },
        _ => Delta::End { index },
    }
}

fn some_ending(rng: &mut Rng) -> (Option<crate::event::Usage>, Option<CallError>) {
    if rng.below(4) == 0 {
        let error = CallError {
            class: ErrorClass::Retryable,
            message: "503".to_string(),
        };
        (None, Some(error))
    } else {
        (None, None)
    }
}

#[test]
fn random_inputs_keep_the_rules() {
    let mut paths = BTreeSet::new();
    for seed in 0..300 {
        let mut rng = Rng(seed);
        let mut session = session();
        let mut watch = Watch::new(seed);
        let mut next_id = 1;
        for _ in 0..60 {
            let input = match rng.below(16) {
                0..=2 => {
                    next_id += 1;
                    send(next_id, "hi")
                }
                3 => send(1 + rng.below(next_id), "hi"),
                4 => {
                    next_id += 1;
                    send(next_id, "")
                }
                5 => Input::Environment(environment(if rng.below(2) == 0 { "~/a" } else { "~/b" })),
                6 => {
                    // 多半是叫过的那个回合，偶尔是对不上的。
                    let turn = match watch.hooked.last() {
                        Some(turn) if rng.below(4) > 0 => *turn,
                        _ => TurnId::new(seq(1 + rng.below(watch.last()))),
                    };
                    hooks_done(turn, some_injections(&mut rng))
                }
                7 => stored(1 + rng.below(watch.last())),
                8 | 9 => stored(watch.last()),
                10 => Input::RequestSent {
                    at: at(40),
                    seen: watch.some_seen(&mut rng),
                    model: Model {
                        endpoint: ProviderId::parse("deepseek").unwrap(),
                        model: ModelName::parse("deepseek-v4").unwrap(),
                    },
                    request: ContentHash::of(b"request"),
                },
                11..=13 => Input::ModelDelta {
                    at: at(41),
                    seen: watch.some_seen(&mut rng),
                    delta: watch.some_delta(&mut rng),
                },
                _ => {
                    let (usage, error) = some_ending(&mut rng);
                    Input::ModelEnded {
                        at: at(45),
                        seen: watch.some_seen(&mut rng),
                        usage,
                        error,
                    }
                }
            };
            watch.feed(&mut session, input);
        }
        let last = watch.last();
        watch.feed(&mut session, stored(last));
        assert_eq!(
            watch.received, watch.replied,
            "种子 {seed}：每收到一次命令要恰好回应一次"
        );
        paths.extend(watch.seen_paths);
    }
    let expected = [
        "推了增量",
        "叫执行器别再发",
        "跑了回合结束的挂接点",
        "说完了",
        "出错了",
        "回复里有工具调用",
        "开了第二轮",
    ];
    for path in expected {
        assert!(paths.contains(path), "三百例里一次都没走到「{path}」");
    }
}
