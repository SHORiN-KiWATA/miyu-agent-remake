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
//!   下一次请求接着上一次往下长；
//! - 改回文件：那几轮改过文件的才交出去，一个改过的文件一步；改的时候来的命令拒绝；结局只记一条
//!   `files.restored`；过时的结局不理（施工 4-7 上）；
//! - 打断时在跑的改文件的调用：叫它停，等它交回来、到点、又打断一次才收尾（施工 4-9 再补一，
//!   `watch/stopping.rs`）；
//! - 压缩：交了限额、用量过了线，先发摘要请求，替代到的 N 照规矩；说完了写压缩，取不出摘要的出错收场；压完的
//!   请求照检查点以后的（施工 6-2 上，`watch/compaction.rs`）；撤销能撤掉压缩：先读回日志，对不上的不理，恢复不读，
//!   检查点换了取回原文（施工 6-9，`watch/undo.rs`、`random/undoing.rs`）；
//! - 手动压缩：照规矩收下或者拒绝；收下的单开一轮，不跑挂接点、不注入，只发摘要请求，写完压缩同一批结束；失败不数进
//!   熔断；被重启打断的不接着干（施工 6-8，`watch/manual.rs`）；
//! - 回报：对不上的拒绝、不理；闲着时开一轮还是只记下，正忙时排着、回合结束时接着开，恢复撤销以后接着开（施工 7-2，
//!   `watch/reports.rs`、`random/reporting.rs`）。
//!
//! 每一步还照九条不变量查（`watch/invariants.rs`，`02-内核.md` 第九节「不变量怎么查」）。
//!
//! 还查自己走到了没有：三百例里每条路至少走到一次，清单上的每一种输入至少喂过一次
//! （`random/kinds.rs`），不然查的是空话。CI 另有一项长跑，接着往后跑两万例。

mod asking;
mod compacting;
mod endings;
mod kinds;
mod paths;
mod reporting;
mod rereading;
mod restoring;
mod rng;
mod stopping;
mod undoing;
mod watch;

use std::collections::BTreeSet;

use super::approval::answer;
use super::permission::{read_only, switch};
use super::question::reply;
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
use compacting::{random_policy, some_compact, some_limits, some_overflow};
use endings::some_ending;
use kinds::InputKind;
use paths::{EXPECTED_PATHS, LONG_PATHS};
use rereading::some_reread;
use restoring::some_restored;
use rng::Rng;
use stopping::some_stop_end;
use undoing::{read_back_now, some_read_back, some_undo};
use watch::Watch;

/// 随机测试的会话，一个回合最多请求几次模型。
const STEP_LIMIT: u32 = 2;

impl Watch {
    /// 下一段增量：多半接着在路上的那次请求像样地往下说，偶尔乱来。工具调用的参数是一个
    /// 空对象，一次写完。新的一块三回里两回是工具调用；手动压缩那一轮只有摘要请求，倒过来，三回里一回（施工 6-8：
    /// 摘要回复里调了工具的取不到摘要，不然难得压成）。
    fn some_delta(&mut self, rng: &mut Rng) -> Delta {
        if rng.below(if self.calm { 40 } else { 6 }) == 0 {
            return scrambled_delta(rng);
        }
        match self.open_block {
            None => {
                let index = self.next_block;
                self.next_block += 1;
                let roll = rng.below(3);
                let tool = match self.manual_turn() {
                    Some(_) => roll == 0,
                    None => roll > 0,
                };
                self.open_block = Some((index, tool, false));
                let kind = if tool {
                    let name = match self.writing {
                        true => ["read", "write", "write"][rng.below(3) as usize],
                        false => ["read", "read", "read", "write", "write", "reed"]
                            [rng.below(6) as usize],
                    };
                    Kind::ToolCall {
                        name: name.to_string(),
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

/// 到点了：为 `seen` 那次请求等的。
fn woke(seen: Seq) -> Input {
    Input::Woke { at: at(55), seen }
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
    // 等着重试的，多半很快到点；偶尔来一个对不上的到点了，该不理（施工 3-5 下）。
    if let Some(seen) = watch.waiting_retry()
        && rng.below(2) == 0
    {
        return woke(seen);
    }
    if rng.below(80) == 0 {
        return woke(watch.some_seen(rng));
    }
    if let Some(input) = some_stop_end(rng, watch, next_id) {
        return input;
    }
    // 写文件的种子：写的调用在跑，常被打断，停着的才走得到（施工 4-9 再补一）。
    if watch.writing && watch.write_running() && rng.below(3) == 0 {
        return some_interrupt(rng, next_id);
    }
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
    if !watch.writing && watch.write_waiting() && rng.below(4) == 0 {
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
        20 | 21 => {
            let (error, wait_ms) = some_ending(rng);
            Input::ModelEnded {
                at: at(45),
                seen: watch.some_seen(rng),
                usage: None,
                error,
                wait_ms,
                excess: None,
            }
        }
        22 => progress(watch.some_call(rng)),
        23..=25 => {
            let call_id = watch.some_call(rng);
            Input::ToolDone {
                at: at(50),
                call_id,
                error: rng.below(4) == 0,
                blocks: Vec::new(),
                duration_ms: Some(1),
                human: None,
                effects: watch.some_effects(),
                // 没叫它停却停了的：在跑的偶尔这样交回，内核照已取消记（施工 4-9 再补一）。
                stopped: watch.running.contains(&call_id) && rng.below(4) == 0,
            }
        }
        26 if !watch.calm || rng.below(10) == 0 => some_interrupt(rng, next_id),
        26 => send(next_command(next_id), "hi"),
        27 if rng.below(2) == 0 => urgent(next_command(next_id), "等等"),
        27 => send(next_command(next_id), "hi"),
        28 => read_only(next_command(next_id), !watch.writing && rng.below(2) == 0),
        _ => switch(next_command(next_id), Some(some_level(rng)), None),
    }
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

/// 跑一段种子，每一例三百条输入，照看守的规矩查（[`watch`]）。返回走到过的路和喂过的输入种类。
fn run(seeds: std::ops::Range<u64>) -> (BTreeSet<&'static str>, BTreeSet<InputKind>) {
    let mut paths = BTreeSet::new();
    let mut fed = BTreeSet::new();
    for seed in seeds {
        let mut rng = Rng(seed);
        // 五个种子里有一个没人能确认。
        let attended = seed % 5 != 4;
        // 三个种子里有一个有隔离式那句 system（施工 6-6 下）：别的调了工具照失败算，连续失败、暂停才走得到。
        let isolate = seed % 3 == 1;
        // 七个种子里有一个是一次性的会话（施工 7-2）：没人看着时回报只记下。
        let oneshot = seed % 7 == 3;
        let mut session = reporting::opened(random_policy(attended, isolate), oneshot);
        let mut watch = Watch::new(seed);
        watch.reports.oneshot = oneshot;
        watch.approvals.attended = attended;
        // 双数的种子风平浪静：打断、乱来的增量少，一轮才走得深；单数的种子专门捣乱。
        watch.calm = seed % 2 == 0;
        // 十个种子里有一个多调写文件的、不开只读，写的在跑时常被打断（施工 4-9 再补一）。
        watch.writing = seed % 10 == 2;
        let mut next_id = 1;
        // 崩不崩、撤不撤另用两串随机数，撤销、恢复夹在原来的输入之间、不占名额：原来那串输入
        // 不跟着错开。
        let mut crashes = Rng(seed ^ 0x00C0_FFEE);
        let mut undos = Rng(seed ^ 0x0DD0_0DD0);
        let mut restores = Rng(seed ^ 0x5E57_04ED);
        let mut limits = Rng(seed ^ 0x11A1_7500);
        let mut rereads = Rng(seed ^ 0x2E2E_AD00);
        let mut overflows = Rng(seed ^ 0x0F10_0D00);
        let mut readbacks = Rng(seed ^ 0x2EAD_BAC0);
        let mut compacts = Rng(seed ^ 0xC0_4AC7);
        let mut reports = Rng(seed ^ 0x2E90_2750);
        for _ in 0..300 {
            if watch.all_stored() && crashes.below(200) == 0 {
                let planned = crashes.below(2) == 0;
                session = watch.reload(session, planned, random_policy(attended, isolate));
                if let Some(input) = watch.recall_answer() {
                    watch.feed(&mut session, input);
                }
                continue;
            }
            if let Some(input) = some_read_back(&mut readbacks, &watch, &mut next_id) {
                watch.feed(&mut session, input);
            }
            if let Some(input) = some_undo(&mut undos, &watch, &mut next_id) {
                watch.feed(&mut session, input);
            }
            if let Some(input) = some_restored(&mut restores, &watch, &mut next_id) {
                watch.feed(&mut session, input);
            }
            if let Some(input) = some_limits(&mut limits) {
                watch.feed(&mut session, input);
            }
            if let Some(input) = some_reread(&mut rereads, &watch) {
                watch.feed(&mut session, input);
            }
            if let Some(input) = some_overflow(&mut overflows, &watch) {
                watch.feed(&mut session, input);
            }
            if let Some(input) = some_compact(&mut compacts, &watch, &mut next_id) {
                watch.feed(&mut session, input);
            }
            if let Some(input) = reporting::some_report(&mut reports, &watch, &mut next_id) {
                watch.feed(&mut session, input);
            }
            let input = some_input(&mut rng, &mut watch, &mut next_id);
            watch.feed(&mut session, input);
        }
        if let Some(input) = read_back_now(&watch) {
            watch.feed(&mut session, input);
        }
        if let Some(steps) = watch.restoring.pending.clone() {
            let files = steps.iter().map(crate::testkit::restored).collect();
            watch.feed(&mut session, Input::Restored { at: at(58), files });
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
    let (paths, _) = run(300..20_300);
    for path in LONG_PATHS {
        assert!(paths.contains(path), "两万例里一次都没走到「{path}」");
    }
}
