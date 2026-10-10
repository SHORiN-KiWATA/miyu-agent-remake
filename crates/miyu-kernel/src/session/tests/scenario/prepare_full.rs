//! 场景：真满了才停（施工 6-11 三补，2026-10-10 终端界面的会话转来项目主人的意思：还有空间就不停，只有窗口真满了才停下等）。
//! 输出预留 6000、余量 10、窗口 7000：压缩线 990，T = 40，G = 60，过了 930 起压。放不下整份输出预留（用量过了 1000）、还放得下
//! 回答的下限 4096（用量不过 2904）的，照常发主请求，回答压到窗口剩下的；过了 2904 才停下等。

use super::prepare::{compactions, two_turns, words};
use super::*;
use crate::event::TransientBody;
use crate::session::Compaction;

/// 输出预留大的替身：回合开始交开着提前压。
fn roomy() -> Stage {
    let make = || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 6000,
            margin: 10,
            line_percent: 100,
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
    stage.limits(Some(7000), None);
    stage
}

/// 推过几次进度。
fn progressed(stage: &Stage) -> usize {
    stage
        .transients()
        .iter()
        .filter(|transient| matches!(transient.body, TransientBody::CompactionProgress(_)))
        .count()
}

/// 每次主请求的回答上限。
fn caps(stage: &Stage) -> Vec<Option<u64>> {
    stage
        .requests()
        .iter()
        .map(|(_, request)| request.output_cap)
        .collect()
}

#[test]
fn without_room_for_the_whole_reserve_it_still_goes_on_with_a_capped_answer() {
    let mut stage = roomy();
    stage.prepare_model([Line::says("P1").held()]);
    two_turns(&mut stage, 920, 1490);
    assert_eq!(stage.prepares().len(), 1, "过了起压线");
    // 第三轮的请求 1506：过了线，放不下整份输出预留，放得下 4096：照发，回答至多 7000 − 1506。
    stage.model([Line::says("好")]);
    let requests = stage.requests().len();
    stage.say(&words(10));
    assert_eq!(stage.requests().len(), requests + 1, "照常发了主请求");
    assert_eq!(progressed(&stage), 0, "不停、不推进度");
    assert!(compactions(&stage).is_empty());
    assert_eq!(
        caps(&stage),
        [None, None, Some(5494)],
        "放得下整份预留的不压"
    );
}

#[test]
fn only_without_room_for_the_least_answer_it_waits() {
    let mut stage = roomy();
    stage.prepare_model([Line::says("P1").held()]);
    two_turns(&mut stage, 920, 2950);
    // 第三轮的请求 2965：加上 4096 超了窗口，停下等在路上的那一次。
    stage.model([Line::says("好")]);
    let requests = stage.requests().len();
    stage.say(&words(10));
    assert_eq!(stage.requests().len(), requests, "不发主请求");
    assert_eq!(progressed(&stage), 1, "等的时候推进度");
}
