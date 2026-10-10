//! 场景：压缩线封在窗口的百分之几，后台起压推 `compaction.started`（施工 6-11 再补，2026-10-10 项目主人定：压缩线是窗口的
//! 85%）。数同 `prepare.rs`，只是窗口 500、压缩线至多窗口的 80%：压缩线 = min(400, 500 − 10 − 10) = 400，T = 40，G = 60，
//! 过了 340 起压；到线以后还能长的那一截是 500 − 10 − 400 = 90，不是余量 10。

use super::prepare::{compactions, done, two_turns, words};
use super::*;
use crate::event::{CompactionStarted, TransientBody};
use crate::session::Compaction;

/// 会提前压的替身：窗口 500，压缩线至多窗口的 80%；回合开始交开着。
fn capped() -> Stage {
    let make = || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            line_percent: 80,
            margin_percent: 100,
            tail: 40,
            lead: 60,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: None,
            pause: None,
            shorten: None,
            isolate: true,
        });
        policy
    };
    let mut stage = Stage::new(make, environment("~/src/miyu"), at(0));
    stage.prepare(true);
    stage.limits(Some(500), None);
    stage
}

/// 推过的 `compaction.started`，和它的回合。
fn started(stage: &Stage) -> Vec<(Option<TurnId>, &CompactionStarted)> {
    stage
        .transients()
        .iter()
        .filter_map(|transient| match &transient.body {
            TransientBody::CompactionStarted(started) => Some((transient.turn, started)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_line_is_a_share_of_the_window_and_starting_in_the_background_is_pushed() {
    let mut stage = capped();
    stage.prepare_model([Line::says("P1")]);
    two_turns(&mut stage, 330, 390);
    // 第二轮的请求 345：不封的压缩线是 480、起压线 420，不起；封在 400，过了 340 起。
    assert_eq!(stage.prepares().len(), 1, "压缩线封在窗口的 80%");
    let pushed = started(&stage);
    assert_eq!(pushed.len(), 1, "起压推一条开始");
    let (turn, body) = pushed[0];
    assert_eq!((body.seen.get(), body.prepared), (8, true));
    assert_eq!(turn, Some(TurnId::new(seq(10))), "在第二轮里");
    // 到线换上：不再推开始。
    stage.model([Line::says("好")]);
    stage.say(&words(10));
    assert_eq!(compactions(&stage).len(), 1);
    assert_eq!(done(&stage), [true]);
    assert_eq!(started(&stage).len(), 1);
}

/// 到线以后接着说，尾巴长过了 T + G 加余量（110），还在 T + G 加到线以后还能长的那一截（190）以内：压好的那份照用。
#[test]
fn the_room_past_the_line_is_up_to_the_window_less_the_reserve() {
    let mut stage = capped();
    stage.prepare_model([Line::says("P1").held()]);
    stage.model([
        Line::says(&words(50)).reports(330),
        Line::says(&words(150)).reports(310),
        Line::says("好").reports(405),
        Line::says("又好"),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
    stage.say(&words(10));
    stage.release_prepare();
    stage.say(&words(2));
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 1, "{:?}", story(&stage));
    assert_eq!(compacted[0].summary, "P1", "照用压好的那份");
    assert_eq!(done(&stage), [true]);
}
