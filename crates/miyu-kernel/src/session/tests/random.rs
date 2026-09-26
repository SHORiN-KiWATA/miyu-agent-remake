//! 随机一串输入：发消息、重发、空消息、落盘、环境变了、挂接点的结果、执行器的三种回报、
//! 工具的结果和输出，对得上的、对不上的、先后乱了的，随机排。每一步查（[`watch`]）：
//!
//! - 序号连着；每收到一次命令恰好回应一次，接受的回应都在它的事件推送以后；
//! - 叫跑挂接点时，回合的开头都落了盘；一个回合只叫一次；
//! - 请求模型时，追加过的事件都落了盘，这个回合的挂接点跑完了，上一步的调用都有了结果；
//!   请求照的是那一刻的全部历史，`seen` 是最后一条；一个回合的请求不超过上限；
//! - 每次请求至多一条 `model.called`：正常说完的排在它的回复后面，出错的后面紧跟着出错的
//!   `turn.ended`；推给头的增量是在路上的那次请求的；叫执行器别再发的，这次请求已经记了出错；
//! - 交给执行前的链时回复落了盘，每个调用只交一次，不是只读的不和别的占着位置的一起、不越过
//!   前面还没结果的，带着回合开始时的工作目录和实际生效的那一级；派去跑的都被链放行过，或者被人
//!   允许过、那条决定也落了盘；每个调用一条结果；
//! - 确认：请求只在链说要问人时记；没人能确认的、只读时要写入的当场拒绝；回答照规矩接受或者
//!   拒绝，接受的记一条决定，`by` 是回答的人；被拒绝的结果，谁拒的写成 `by`；
//! - 提问：题目只由在跑的调用问；回答照规矩接受或者拒绝，落了盘才交给工具；来了一句话作废、打断、
//!   没人能回答，各自写对 `by` 和那一句；
//! - 回合结束的挂接点，等 `turn.ended` 落了盘才跑，一个回合一次；
//! - 偶尔崩一下，或者有计划地重启一下，从落了盘的日志载入：崩了的那一轮收尾、不接着开，重启打断
//!   的接着开一轮；
//! - 切权限级别：只读生效的时候不派写文件的调用，内核拦下的都是写文件的；回合中途注入的排在
//!   这一步的全部工具结果后面；请求时最近一块权限事实写的是现在的那一级，环境那一块写的是
//!   这一轮的工作目录；
//! - 撤销、恢复：照规矩接受或者拒绝，列的是那几轮；请求照的是撤销、恢复以后的历史；撤了又恢复的，
//!   下一次请求接着上一次往下长。
//!
//! 每一步还照九条不变量查（`watch/invariants.rs`，`02-内核.md` 第九节「不变量怎么查」）。
//!
//! 还查自己走到了没有：三百例里每条路至少走到一次，清单上的每一种输入至少喂过一次
//! （`random/kinds.rs`），不然查的是空话。CI 另有一项长跑，接着往后跑两万例。

mod asking;
mod kinds;
mod watch;

use std::collections::BTreeSet;

use super::approval::answer;
use super::permission::{read_only, switch};
use super::question::reply;
use super::revert::{revert, unrevert};
use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::{
    CallError, CallResult, Choice, Decision, EndReason, ErrorClass, Level, Question, Response,
    ToolStatus, Transient, TransientBody,
};
use crate::id::{CallId, ContentHash, FactKind, ModelName, ModuleId, ProviderId};
use crate::origin::Model;
use crate::raw::RawJson;
use crate::tool::Access;
use asking::{some_answer, some_question, some_reply, some_verdict};
use kinds::InputKind;
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
                let tool = rng.below(3) > 0;
                self.open_block = Some((index, tool, false));
                let kind = if tool {
                    Kind::ToolCall {
                        name: ["read", "read", "read", "write", "write", "reed"]
                            [rng.below(6) as usize]
                            .to_string(),
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

/// 一条随机的输入，照下面的权重抽（一共 30 份）。执行器替身多半守规矩：请求交给它以后，
/// 先报发出去了，再送增量和结局；交给了链的，三回里有两回先送回它的结论；有在等人确认的，两回里有
/// 一回先回答（[`some_verdict`]、[`some_answer`]）；有问着人的，四回里有一回回答，捣乱的种子里还有
/// 八回里三回打断（[`some_reply`]）；工具在跑的时候偶尔问人（[`some_question`]）。
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
/// | 1 | 开关只读 |
/// | 1 | 改常用的那一级，偶尔是不认识的 |
fn some_input(rng: &mut Rng, watch: &mut Watch, next_id: &mut u64) -> Input {
    // 交给了链的，多半很快有结论；在等人的，偶尔回答。
    if !watch.approvals.guarding.is_empty() && rng.below(3) > 0 {
        return some_verdict(rng, watch);
    }
    if !watch.approvals.asking.is_empty() && rng.below(2) == 0 {
        return some_answer(rng, watch, next_id);
    }
    if !watch.questions.asking.is_empty() {
        match rng.below(8) {
            0 | 1 => return some_reply(rng, watch, next_id),
            2..=4 if !watch.calm => return some_interrupt(rng, next_id),
            _ => {}
        }
    }
    // 工具在跑的时候，偶尔打断、多送几段输出、问人（没人能回答的种子里多问几回）；有还没派的写文件
    // 调用时，偶尔开只读。这几个窗口都短，光靠均匀地抽难得碰上。
    if !watch.running.is_empty() {
        let asks = if watch.approvals.attended {
            5..=9
        } else {
            5..=14
        };
        match rng.below(30) {
            0 | 1 if !watch.calm => return some_interrupt(rng, next_id),
            1..=4 => return progress(watch.some_call(rng)),
            k if asks.contains(&k) => return some_question(rng, watch),
            _ => {}
        }
    }
    if watch.write_waiting() && rng.below(4) == 0 {
        return read_only(next_command(next_id), true);
    }
    let slot = rng.below(30);
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
        26 if !watch.calm || rng.below(10) == 0 => some_interrupt(rng, next_id),
        26 => send(next_command(next_id), "hi"),
        27 if rng.below(2) == 0 => urgent(next_command(next_id), "等等"),
        27 => send(next_command(next_id), "hi"),
        28 => read_only(next_command(next_id), rng.below(2) == 0),
        _ => switch(next_command(next_id), Some(some_level(rng)), None),
    }
}

/// 撤销、恢复，另用一串随机数：原来那串输入不跟着错开。空闲时四回里有一回，回合开着时五十回里
/// 一回（该被拒）。能恢复的时候一半是恢复，不能的时候十回里一回（该被拒）；撤销多半从还在有效历史
/// 里的最后三轮之一起，偶尔是对不上的。
fn some_undo(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Option<Input> {
    let chance = if watch.turn_open() { 50 } else { 4 };
    if rng.below(chance) != 0 {
        return None;
    }
    let n = next_command(next_id);
    let redo = match watch.undo.can_unrevert() {
        true => rng.below(2) == 0,
        false => rng.below(10) == 0,
    };
    if redo {
        return Some(unrevert(n));
    }
    let effective = &watch.undo.effective;
    let turn = match effective.len() {
        k if k > 0 && rng.below(6) > 0 => effective[k - 1 - rng.below(k.min(3) as u64) as usize]
            .started()
            .get(),
        _ => 1 + rng.below(watch.last()),
    };
    Some(revert(n, turn))
}

/// 常用的那一级：多半是认识的，偶尔是不认识的，要被拒绝。
fn some_level(rng: &mut Rng) -> Level {
    match rng.below(5) {
        0 => Level::Other("root".to_string()),
        1 | 2 => Level::Full,
        _ => Level::Workspace,
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

/// 一次打断：一半接着发，一半退回。
fn some_interrupt(rng: &mut Rng, next_id: &mut u64) -> Input {
    let n = next_command(next_id);
    if rng.below(2) == 0 {
        interrupt(n)
    } else {
        take_back(n)
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

/// 随机测试的策略：一个回合最多请求 [`STEP_LIMIT`] 次；`attended` 是有没有人能确认、回答。
fn random_policy(attended: bool) -> Policy {
    let mut limited = policy();
    limited.step_limit = Some(STEP_LIMIT);
    limited.attended = attended;
    limited
}

/// 三百例里每条都要走到的路。
const EXPECTED_PATHS: &[&str] = &[
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
    "排队的接着开了一轮",
    "打断后排队的接着发",
    "打断后排队的退回",
    "回复到了只读拦下",
    "收紧时拦下还没派的",
    "要写入的请求只读拦下",
    "切了级别以后注入",
    "要问人",
    "人允许了",
    "人拒绝了",
    "链拒绝了",
    "没人能确认被拒",
    "回答被拒",
    "工具问人",
    "人回答了",
    "回答交给了工具",
    "回答对不上被拒",
    "来了一句话作废",
    "打断时在等人回答",
    "没人能回答",
    "崩了以后收尾",
    "有计划地重启",
    "重启后接着干",
    "撤销了",
    "撤销被拒",
    "撤销带走了上一轮排着的",
    "恢复了",
    "恢复被拒",
    "恢复以后接着说",
];

/// 跑一段种子，每一例三百条输入，照看守的规矩查（[`watch`]）。返回走到过的路和喂过的输入种类。
fn run(seeds: std::ops::Range<u64>) -> (BTreeSet<&'static str>, BTreeSet<InputKind>) {
    let mut paths = BTreeSet::new();
    let mut fed = BTreeSet::new();
    for seed in seeds {
        let mut rng = Rng(seed);
        // 五个种子里有一个没人能确认。
        let attended = seed % 5 != 4;
        let mut session = session_with(random_policy(attended));
        let mut watch = Watch::new(seed);
        watch.approvals.attended = attended;
        // 双数的种子风平浪静：打断、乱来的增量少，一轮才走得深；单数的种子专门捣乱。
        watch.calm = seed % 2 == 0;
        let mut next_id = 1;
        // 崩不崩、撤不撤另用两串随机数，撤销、恢复夹在原来的输入之间、不占名额：原来那串输入
        // 不跟着错开。
        let mut crashes = Rng(seed ^ 0x00C0_FFEE);
        let mut undos = Rng(seed ^ 0x0DD0_0DD0);
        for _ in 0..300 {
            if watch.all_stored() && crashes.below(200) == 0 {
                let planned = crashes.below(2) == 0;
                session = watch.reload(session, planned, random_policy(attended));
                continue;
            }
            if let Some(input) = some_undo(&mut undos, &watch, &mut next_id) {
                watch.feed(&mut session, input);
            }
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
        fed.extend(watch.fed);
    }
    (paths, fed)
}

/// 平时跑的三百例。还查自己走到了没有：每条路至少走到一次，清单上的每一种输入至少喂过一次
/// （`random/kinds.rs`），不然查的是空话。
#[test]
fn random_inputs_keep_the_rules() {
    let (paths, fed) = run(0..300);
    for path in EXPECTED_PATHS {
        assert!(paths.contains(path), "三百例里一次都没走到「{path}」");
    }
    for kind in InputKind::ALL {
        assert!(fed.contains(kind), "三百例里一次都没喂过「{kind:?}」");
    }
}

/// 长跑：接着平时的往后跑两万例（`docs/designs/02-内核.md` 第九节「不变量怎么查」）。平时的
/// `cargo test` 跳过它，CI 的长跑那一项用 `--ignored`、release 模式跑。
#[test]
#[ignore = "长跑，CI 的长跑那一项用 --ignored 跑（施工 2-10）"]
fn random_inputs_keep_the_rules_for_longer() {
    run(300..20_300);
}
