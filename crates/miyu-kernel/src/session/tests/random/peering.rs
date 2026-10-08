//! 随机测试里别的会话发来的话（施工 C-2，`docs/blueprint/cross-session.md` 第四条、第五条）：另用一串随机数送，夹在原来的
//! 输入之间、不占名额，原来那串输入不跟着错开。三个发话方、四句话，时刻在 07:00 那一分钟里随便取；随机的策略把防刷屏的
//! 数调小（[`LIMITS`]），一字不差、限速、没听到的上限、窗口过了又收都走得到。
//!
//! 「空了告诉我」（施工 C-6）也在这几个种子里：在跑的调用随便交回订了一个发话方（随机的会话难得有调用做完），再随便送
//! 它们交来的「空了」、执行器交的作废和不在了。随机的策略订了就到点（`watch_hours` 是 0），时刻在那一分钟里随便
//! 取，到了点、没到点、不在等、已经等到过都走得到。

use super::*;
use crate::block::Text;
use crate::origin::Session as Peer;

/// 随机测试的防刷屏的数：一个窗口里最多 3 句，窗口 20 秒，没听到的最多 5 句。订了就到点，通知那一行最多 20 个字。
pub(super) const LIMITS: Peers = Peers {
    burst: 3,
    window: 20,
    unread: 5,
    watch_hours: 0,
    status_chars: 20,
};

/// 三个发话方：都不是这个会话派的子代理（子代理的会话见 `watch/reports.rs`）。也是订的那几个会话（施工 C-6）。
const SENDERS: [&str; 3] = [
    "0192f3a0-1111-7abc-8def-001122334455",
    "0192f3a0-2222-7abc-8def-5566778899aa",
    "0192f3a0-3333-7abc-8def-0c5d77aa0c5d",
];

/// 四句话。
const WORDS: [&str; 4] = ["迁移写完了。", "测试过了。", "导出接上了。", "还有一件。"];

/// 别的会话发来一句：正忙时六回里一回，闲着十回里一回。四回里一回带着急着插话的记号（内核不看）。
pub(super) fn some_peer(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Option<Input> {
    let chance = if watch.turn_open() { 6 } else { 10 };
    if rng.below(chance) != 0 {
        return None;
    }
    let from = SENDERS[rng.below(3) as usize];
    let words = WORDS[rng.below(4) as usize];
    Some(Input::Command(Received {
        id: id(next_command(next_id)),
        by: By::Session(Peer {
            id: SessionId::parse(from).unwrap(),
        }),
        at: at(rng.below(60)),
        command: Command::Send {
            blocks: vec![Block::Text(Text {
                text: words.to_string(),
            })],
            urgent: rng.below(4) == 0,
            venue: None,
        },
    }))
}

/// 在跑的调用做完了、订了一个发话方，等的会话交来「空了」，执行器交来作废或者不在了（施工 C-6）。四分之一的有别的会话的
/// 种子（种子除以 16 余 15）有在跑的调用就由它做完、订一个（随机的会话难得有调用在跑）；别的种子不订，原来走得到的路照样
/// 走得到。别的时候六回里一回送通知：八回里六回是「空了」，作废、不在了各一回。订的种子少，收下、作废、不在了这几条路放在
/// 长跑里查（`paths.rs`）。
pub(super) fn some_notice(rng: &mut Rng, watch: &Watch, ids: &mut u64) -> Option<Input> {
    // 只订、只等前两个：第三个一直是不在等的。
    let session = SessionId::parse(SENDERS[rng.below(2) as usize]).unwrap();
    let when = at(rng.below(60));
    if let Some(&call_id) = watch.running.iter().next()
        && watch.seed % 16 == 15
    {
        // 照随机输入里做完的调用那一刻（`random.rs`）。
        return Some(Input::ToolDone {
            at: at(50),
            call_id,
            error: false,
            blocks: Vec::new(),
            duration_ms: Some(1),
            human: None,
            effects: vec![crate::event::Effect::PeerWatch(crate::event::PeerWatch {
                session,
            })],
            stopped: false,
        });
    }
    if rng.below(6) != 0 {
        return None;
    }
    Some(match rng.below(8) {
        0..=5 => Input::Command(Received {
            id: CommandId::parse(&format!("idle-{}", next_command(ids))).unwrap(),
            by: By::Session(Peer { id: session }),
            at: when,
            command: Command::PeerIdle {
                status: (rng.below(2) == 0).then(|| "测试全过了。".to_string()),
            },
        }),
        6 => Input::WatchEnded {
            at: when,
            session,
            reason: crate::event::IdleReason::Expired,
        },
        _ => Input::WatchEnded {
            at: when,
            session,
            reason: crate::event::IdleReason::Gone,
        },
    })
}
