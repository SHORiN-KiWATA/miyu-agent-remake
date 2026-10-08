//! 场景：提前压好、到线换上（`docs/blueprint/compaction.md` 第十五条，施工 6-11 上）。
//!
//! 压缩线 400（窗口 420，输出预留、余量各 10），尾巴的预算 T = 40，提前量 G = min(60, 100 − 40) = 60：用量过了 340 起压。
//! 替身的组装一条事件一行，用量 = 锚 + 新的几行（一行约五个 token）；尾巴照事件的内容估（字节除以 4）。

use super::*;
use crate::event::{Body, CompactTrigger, ContextCompacted, Purpose, TransientBody};
use crate::session::Compaction;

/// 会提前压的替身：输出预留、余量各 10，尾巴的上限 40、提前量的上限 `lead`；回合开始交开着。
pub(super) fn preparing(lead: u64) -> Stage {
    let make = move || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            tail: 40,
            lead,
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
    stage.limits(Some(420), None);
    stage
}

/// `tokens` 个 token 的一段话：四个字节一个。
pub(super) fn words(tokens: usize) -> String {
    "abcd".repeat(tokens)
}

/// 第一轮说 50、回 50（报 `first`），第二轮说 10、回 10（报 `second`）。第二轮的请求用量是 `first` 加 15（新的三行），第三轮的是 `second` 加 15。
/// 日志：2 第一句、3 开回合、4、5 两块事实、6 回复、7、8；9 第二句、10、11 回复、12、13。
pub(super) fn two_turns(stage: &mut Stage, first: u64, second: u64) {
    stage.model([
        Line::says(&words(50)).reports(first),
        Line::says(&words(10)).reports(second),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
}

/// 写下的压缩。
pub(super) fn compactions(stage: &Stage) -> Vec<&ContextCompacted> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .collect()
}

/// 推过的压好了：带没带 `prepared`。
pub(super) fn done(stage: &Stage) -> Vec<bool> {
    stage
        .transients()
        .iter()
        .filter_map(|transient| match &transient.body {
            TransientBody::CompactionDone(done) => Some(done.prepared),
            _ => None,
        })
        .collect()
}

/// 推过几次进度。
pub(super) fn progress(stage: &Stage) -> usize {
    stage
        .transients()
        .iter()
        .filter(|transient| matches!(transient.body, TransientBody::CompactionProgress(_)))
        .count()
}

#[test]
fn past_the_start_line_it_prepares_and_at_the_line_it_swaps_without_asking() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1")]);
    two_turns(&mut stage, 330, 390);
    // 第二轮的请求过了起压线：照现在的切法切到第一轮的末尾，旁路发摘要请求，主请求照发。
    assert_eq!(stage.prepares().len(), 1);
    let (upto, request) = &stage.prepares()[0];
    assert_eq!(upto.get(), 8);
    let listed = listed_request(request);
    assert!(listed.ends_with("8 turn.ended\nsummarize\n"), "{listed}");
    let called: Vec<_> = stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ModelCalled(called) if called.purpose == Some(Purpose::Compaction) => {
                Some((event.turn, called))
            }
            _ => None,
        })
        .collect();
    assert_eq!(called.len(), 1);
    let (turn, called) = called[0];
    assert_eq!(turn, None, "不带回合编号");
    assert_eq!(called.seen.get(), 8);
    assert_eq!(called.compaction, None, "不带 compaction");
    assert!(compactions(&stage).is_empty(), "到线以前一个字节都不改");
    // 第三轮过了线：不再请求，换上那一份。
    stage.model([Line::says("好")]);
    let requests = stage.requests().len();
    stage.say(&words(10));
    assert_eq!(stage.requests().len(), requests + 1, "只多了一次主请求");
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 1);
    assert_eq!(
        (compacted[0].upto.get(), compacted[0].summary.as_str()),
        (8, "P1")
    );
    assert_eq!(compacted[0].trigger, Some(CompactTrigger::Auto));
    assert_eq!(done(&stage), [true], "推的压好了带 prepared");
    assert_eq!(progress(&stage), 0, "没推进度");
    let (_, main) = stage.requests().last().unwrap();
    assert!(
        listed_request(main).starts_with("9 message.user\n"),
        "{}",
        listed_request(main)
    );
}

#[test]
fn below_the_start_line_switched_off_or_without_lead_nothing_is_prepared() {
    let mut stage = preparing(60);
    two_turns(&mut stage, 320, 300);
    assert!(stage.prepares().is_empty(), "335 没过起压线 340");
    let mut stage = preparing(60);
    stage.prepare(false);
    two_turns(&mut stage, 330, 300);
    assert!(stage.prepares().is_empty(), "开关关着");
    let mut stage = preparing(0);
    two_turns(&mut stage, 330, 300);
    assert!(stage.prepares().is_empty(), "G 是 0");
    // 压缩线不到 160 的：四分之一比尾巴的上限 40 还小，G 是 0。线 150，第二轮 135，照理离线只有 15。
    let mut stage = preparing(60);
    stage.limits(Some(170), None);
    two_turns(&mut stage, 120, 100);
    assert!(stage.prepares().is_empty(), "小窗口不提前压");
}

#[test]
fn only_one_is_prepared_at_a_time() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1").held()]);
    two_turns(&mut stage, 330, 345);
    assert_eq!(stage.prepares().len(), 1);
    // 在路上：第三轮也过了起压线，不再发。
    stage.model([Line::says(&words(10)).reports(350)]);
    stage.say(&words(10));
    assert_eq!(stage.prepares().len(), 1, "在路上的不再发");
    // 压好了、还用得上：第四轮不再发。
    stage.release_prepare();
    stage.model([Line::says(&words(10)).reports(360)]);
    stage.say(&words(10));
    assert_eq!(stage.prepares().len(), 1, "压好了的不再发");
    assert!(compactions(&stage).is_empty());
}

#[test]
fn a_tail_grown_past_t_plus_g_compacts_as_before() {
    // 第二轮回了 120 个 token：N 以后的尾巴 140，超了 T + G = 100。
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1")]);
    stage.model([
        Line::says(&words(50)).reports(330),
        Line::says(&words(120)).reports(390),
        Line::says("S1"),
        Line::says("好"),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
    assert_eq!(stage.prepares().len(), 1);
    stage.say(&words(10));
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 1);
    assert_eq!(compacted[0].summary, "S1");
    assert_eq!(done(&stage), [false]);
}
