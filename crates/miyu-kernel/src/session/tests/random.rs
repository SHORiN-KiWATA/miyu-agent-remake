//! 随机一串输入：发消息、重发、空消息、落盘、环境变了、挂接点的结果、执行器的三种回报、
//! 工具的结果和输出，对得上的、对不上的、先后乱了的，随机排。每一步查（[`watch`]）：
//!
//! - 序号连着；每收到一次命令恰好回应一次，接受的回应都在它的事件推送以后；
//! - 叫跑挂接点时，回合的开头都落了盘；一个回合只叫一次；
//! - 请求模型时，追加过的事件都落了盘，这个回合的挂接点跑完了，上一步的调用都有了结果；
//!   请求照的是那一刻的全部历史，`seen` 是最后一条；一个回合的请求不超过上限；
//! - 每次请求至多一条 `model.called`：正常说完的排在它的回复后面，出错的后面紧跟着出错的
//!   `turn.ended`；推给头的增量是在路上的那次请求的；叫执行器别再发的，这次请求已经记了出错；
//! - 派工具时回复落了盘，每个调用只派一次，不是只读的不和别的一起跑、不越过前面还没结果的，
//!   带着回合开始时的工作目录；每个调用一条结果；
//! - 回合结束的挂接点，等 `turn.ended` 落了盘才跑，一个回合一次。
//!
//! 还查自己走到了没有：三百例里每条路至少走到一次，不然查的是空话。

mod watch;

use std::collections::BTreeSet;

use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::{
    CallError, CallResult, EndReason, ErrorClass, ToolStatus, Transient, TransientBody,
};
use crate::id::{CallId, ContentHash, FactKind, ModelName, ModuleId, ProviderId};
use crate::origin::Model;
use watch::Watch;

/// 随机测试的会话，一个回合最多请求几次模型。
const STEP_LIMIT: u32 = 2;

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

impl Watch {
    /// 下一段增量：多半接着在路上的那次请求像样地往下说，偶尔乱来。工具调用的参数是一个
    /// 空对象，一次写完。
    fn some_delta(&mut self, rng: &mut Rng) -> Delta {
        if rng.below(if self.calm { 40 } else { 6 }) == 0 {
            return scrambled_delta(rng);
        }
        match self.open_block {
            None => {
                let index = self.next_block;
                self.next_block += 1;
                let tool = rng.below(2) == 0;
                self.open_block = Some((index, tool, false));
                let kind = if tool {
                    Kind::ToolCall {
                        name: ["read", "read", "write", "reed"][rng.below(4) as usize].to_string(),
                    }
                } else {
                    Kind::Text
                };
                Delta::Start { index, kind }
            }
            Some((index, true, false)) => {
                self.open_block = Some((index, true, true));
                Delta::Text {
                    index,
                    text: "{}".to_string(),
                }
            }
            Some((index, true, true)) => {
                self.open_block = None;
                Delta::End { index }
            }
            Some((index, false, _)) if rng.below(3) == 0 => {
                self.open_block = None;
                Delta::End { index }
            }
            Some((index, false, _)) => Delta::Text {
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

    /// 多半是在跑的一个调用，偶尔是对不上的。
    fn some_call(&self, rng: &mut Rng) -> CallId {
        let running: Vec<CallId> = self.running.iter().copied().collect();
        match running.len() {
            0 => CallId::new(seq(1 + rng.below(self.last())), 1).unwrap(),
            n if rng.below(5) > 0 => running[rng.below(n as u64) as usize],
            _ => CallId::new(seq(1 + rng.below(self.last())), 1 + rng.below(3) as u32).unwrap(),
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

fn some_ending(rng: &mut Rng) -> Option<CallError> {
    (rng.below(4) == 0).then(|| CallError {
        class: ErrorClass::Retryable,
        message: "503".to_string(),
    })
}

/// 一条随机的输入，照下面的权重抽（一共 28 份）。执行器替身多半守规矩：请求交给它以后，
/// 先报发出去了，再送增量和结局。
///
/// | 份数 | 输入 |
/// |---|---|
/// | 3 | 发一条新消息 |
/// | 1 | 重发一个用过的编号 |
/// | 1 | 空消息 |
/// | 1 | 环境变了 |
/// | 2 | 回合开始的挂接点跑完了 |
/// | 1 | 落盘到随便哪一条 |
/// | 4 | 全落盘 |
/// | 1 | 请求发出去了 |
/// | 6 | 模型的一段增量 |
/// | 2 | 模型说完了 |
/// | 1 | 工具的输出 |
/// | 3 | 工具执行完了 |
/// | 1 | 打断 |
/// | 1 | 一半急着插话，一半发新消息：急着插话一来，这一轮回复里的调用就全跳过，不能多 |
fn some_input(rng: &mut Rng, watch: &mut Watch, next_id: &mut u64) -> Input {
    // 工具在跑的时候，偶尔打断、多送几段输出：这个窗口短，光靠均匀地抽难得碰上。
    if !watch.running.is_empty() {
        match rng.below(30) {
            0 if !watch.calm => return interrupt(next_command(next_id)),
            1..=4 => return progress(watch.some_call(rng)),
            _ => {}
        }
    }
    let slot = rng.below(28);
    if (13..=21).contains(&slot)
        && let Some(seen) = watch.unsent()
        && rng.below(5) > 0
    {
        return sent_now(seen);
    }
    match slot {
        0..=2 => send(next_command(next_id), "hi"),
        3 => send(1 + rng.below(*next_id), "hi"),
        4 => send(next_command(next_id), ""),
        5 => Input::Environment(environment(if rng.below(2) == 0 { "~/a" } else { "~/b" })),
        6 | 7 => {
            // 多半是叫过的那个回合，偶尔是对不上的。
            let turn = match watch.hooked.last() {
                Some(turn) if rng.below(4) > 0 => *turn,
                _ => TurnId::new(seq(1 + rng.below(watch.last()))),
            };
            hooks_done(turn, some_injections(rng))
        }
        8 => stored(1 + rng.below(watch.last())),
        9..=12 => stored(watch.last()),
        13 => sent_now(watch.some_seen(rng)),
        14..=19 => Input::ModelDelta {
            at: at(41),
            seen: watch.some_seen(rng),
            delta: watch.some_delta(rng),
        },
        20 | 21 => Input::ModelEnded {
            at: at(45),
            seen: watch.some_seen(rng),
            usage: None,
            error: some_ending(rng),
        },
        22 => progress(watch.some_call(rng)),
        23..=25 => Input::ToolDone {
            at: at(50),
            call_id: watch.some_call(rng),
            error: rng.below(4) == 0,
            blocks: Vec::new(),
            duration_ms: Some(1),
        },
        26 if !watch.calm || rng.below(10) == 0 => interrupt(next_command(next_id)),
        26 => send(next_command(next_id), "hi"),
        _ if rng.below(2) == 0 => urgent(next_command(next_id), "等等"),
        _ => send(next_command(next_id), "hi"),
    }
}

/// 调用 `call_id` 执行中的一段输出。
fn progress(call_id: CallId) -> Input {
    Input::ToolProgress {
        at: at(49),
        call_id,
        text: "…".to_string(),
    }
}

/// 下一个没用过的命令编号。
fn next_command(next_id: &mut u64) -> u64 {
    *next_id += 1;
    *next_id
}

/// 请求 `seen` 发出去了。
fn sent_now(seen: Seq) -> Input {
    Input::RequestSent {
        at: at(40),
        seen,
        model: Model {
            endpoint: ProviderId::parse("deepseek").unwrap(),
            model: ModelName::parse("deepseek-v4").unwrap(),
        },
        request: ContentHash::of(b"request"),
    }
}

#[test]
fn random_inputs_keep_the_rules() {
    let mut paths = BTreeSet::new();
    for seed in 0..300 {
        let mut rng = Rng(seed);
        let mut limited = policy();
        limited.step_limit = Some(STEP_LIMIT);
        let mut session = session_with(limited);
        let mut watch = Watch::new(seed);
        // 双数的种子风平浪静：打断、乱来的增量少，一轮才走得深；单数的种子专门捣乱。
        watch.calm = seed % 2 == 0;
        let mut next_id = 1;
        for _ in 0..150 {
            let input = some_input(&mut rng, &mut watch, &mut next_id);
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
        "派了工具",
        "推了工具的输出",
        "一步接一步",
        "走到步数上限",
        "打断了回合",
        "打断了请求",
        "打断了工具",
        "空闲时打断被拒",
        "急着插话跳过",
    ];
    for path in expected {
        assert!(paths.contains(path), "三百例里一次都没走到「{path}」");
    }
}
