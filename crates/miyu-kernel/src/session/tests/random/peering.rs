//! 随机测试里别的会话发来的话（施工 C-2，`docs/blueprint/cross-session.md` 第四条、第五条）：另用一串随机数送，夹在原来的
//! 输入之间、不占名额，原来那串输入不跟着错开。三个发话方、四句话，时刻在 07:00 那一分钟里随便取；随机的策略把防刷屏的
//! 数调小（[`LIMITS`]），一字不差、限速、没听到的上限、窗口过了又收都走得到。

use super::*;
use crate::block::Text;
use crate::origin::Session as Peer;

/// 随机测试的防刷屏的数：一个窗口里最多 3 句，窗口 20 秒，没听到的最多 5 句。
pub(super) const LIMITS: Peers = Peers {
    burst: 3,
    window: 20,
    unread: 5,
};

/// 三个发话方：都不是这个会话派的子代理（子代理的会话见 `watch/reports.rs`）。
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
        },
    }))
}
