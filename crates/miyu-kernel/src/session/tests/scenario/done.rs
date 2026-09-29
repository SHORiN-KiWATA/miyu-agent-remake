//! 场景：压好了（施工 6-3 下）。压完、组装这一步第一次主请求时推一条 `compaction.done`：压前是过线的那一次算出的，压后
//! 是压完这一次算出的，摘要请求的用量、用时取自它的 `model.called`。被打断、出错的不推。

use super::*;
use crate::event::{CompactionDone, TransientBody};
use crate::session::Compaction;

/// 会压缩的替身：输出预留、余量各 10，尾巴 0。
fn compacting() -> Stage {
    let make = || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            tail: 0,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: None,
            pause: None,
            shorten: None,
        });
        policy
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// 推过的压好了，照先后。
fn done(stage: &Stage) -> Vec<&CompactionDone> {
    stage
        .transients()
        .iter()
        .filter_map(|transient| match &transient.body {
            TransientBody::CompactionDone(done) => Some(done),
            _ => None,
        })
        .collect()
}

/// 第一轮答一句（报 `reported`），线 100；第二轮一开头就压。
fn second_turn(stage: &mut Stage, reported: u64, summary: Line) {
    stage.model([Line::says("好。").reports(reported)]);
    stage.say("hi");
    stage.limits(Some(120), None);
    stage.model([summary, Line::says("嗯。")]);
    stage.say("再说一句");
}

#[test]
fn done_carries_the_usage_before_and_after() {
    let mut stage = compacting();
    second_turn(&mut stage, 5_000, Line::says("S1").reports(700));
    let pushed = done(&stage);
    assert_eq!(pushed.len(), 1);
    let done = pushed[0];
    assert_eq!(done.seen, seq(8));
    // 压前：锚报的 5000 加上后来的几条；压后：检查点、第二句和两块事实，替身的组装一条几个 token。
    assert!((5_000..5_100).contains(&done.before), "{}", done.before);
    assert!(done.after < 100, "{}", done.after);
    assert_eq!(done.usage.as_ref().map(|usage| usage.uncached), Some(700));
    assert!(done.duration_ms.is_some());
    // 推在写压缩的那一批以后、主请求以前。
    let calls = stage.model_calls();
    assert_eq!(calls.len(), 3);
}

#[test]
fn nothing_is_done_when_the_summary_fails_or_is_interrupted() {
    let mut stage = compacting();
    second_turn(&mut stage, 5_000, Line::says("").thinking("只想了"));
    assert!(done(&stage).is_empty(), "取不出摘要");
    let mut stage = compacting();
    stage.model([Line::says("好。").reports(5_000)]);
    stage.say("hi");
    stage.limits(Some(120), None);
    stage.model([Line::says("S1").held()]);
    stage.say("再说一句");
    stage.interrupt(Queued::Send);
    assert!(done(&stage).is_empty(), "被打断");
}
