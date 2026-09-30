//! 场景：截短重试（`docs/blueprint/compaction.md` 第三条第 10 条，施工 6-6 中）。摘要请求报超长，截掉检查点后面最老的
//! 几组，落了盘再发；截过的最前面一行写着截到哪（替身的组装）；截够了、截不动的照一次压缩失败算。
//!
//! 第一轮没有线，调几次工具再答：人的那句、每次调工具、最后的回答各一组（两次的是四组，序号 2、6、9、12 开头）。第二轮
//! 交压缩线 100，一开头就过线，替代到第一轮的最后一条。

use super::*;
use crate::event::CallResult;
use crate::session::{Compaction, Notes, Shorten};
use crate::template::Template;

/// 截短重试照出厂的：再试 3 次，没说超多少的去掉 20%。
const SHORTEN: Shorten = Shorten {
    tries: 3,
    percent: 20,
};

/// 一个会压缩、会截短的替身；`shorten` 没有的，快照里没有截短的数。
fn shortening(shorten: Option<Shorten>) -> Stage {
    let make = move || {
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
            shorten,
            isolate: false,
        });
        let template = |source: &str| Template::parse(source).unwrap();
        policy.notes = Some(Notes {
            files: template(""),
            files_more: template(""),
            retrieve: template("<retrieve {upto}/>"),
            too_large: template(""),
            uncovered: Some(template("<uncovered {from}-{to}/>")),
            running: None,
        });
        policy
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// 第一轮：调 `reads` 次工具再答，`reads + 2` 组，每组 3 条（开头的人那一组 4 条）。之后交压缩线 100：第二轮一开头就压。
fn groups(stage: &mut Stage, reads: usize) {
    let mut lines: Vec<Line> = (0..reads)
        .map(|_| Line::calls("", &[("read", "{}")]))
        .collect();
    lines.push(Line::says("好。"));
    stage.model(lines);
    stage.tools((0..reads).map(|k| Play::Done(format!("{k}.rs"))));
    stage.say("看看");
    assert_eq!(stage.log().len(), 8 + 3 * reads);
    stage.limits(Some(120), None);
}

/// 四组：2、6、9、12 开头，到 14 为止。
fn four_groups(stage: &mut Stage) {
    groups(stage, 2);
}

/// 摘要请求报超长。
fn too_long() -> Line {
    Line::fails(ErrorClass::ContextTooLong, "413")
}

/// 摘要请求，照先后：替代到哪、截到哪（没截的没有）。
fn summaries(stage: &Stage) -> Vec<(u64, Option<u64>)> {
    stage
        .requests()
        .iter()
        .map(|(seen, request)| (seen.get(), listed_request(request)))
        .filter(|(_, listed)| listed.ends_with("summarize\n"))
        .map(|(seen, listed)| {
            let cut = listed
                .lines()
                .next()
                .and_then(|line| line.strip_prefix("truncated after "))
                .map(|cut| cut.parse().unwrap());
            (seen, cut)
        })
        .collect()
}

/// 估算第 `from` 到第 `to` 条（含）加起来有多少 token：和内核截短时同一个数法。
fn size(stage: &Stage, from: u64, to: u64) -> u64 {
    let price = crate::estimate::Flat {
        image: 50,
        file: 50,
    };
    stage
        .log()
        .iter()
        .filter(|event| (from..=to).contains(&event.seq.get()))
        .map(|event| crate::estimate::event(event, &price))
        .sum()
}

/// 日志里最后一次压缩。
fn last_compaction(stage: &Stage) -> Option<crate::event::ContextCompacted> {
    stage
        .log()
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted.clone()),
            _ => None,
        })
}

#[test]
fn without_a_count_it_drops_a_fifth_of_the_groups_and_goes_again() {
    let mut stage = shortening(Some(SHORTEN));
    four_groups(&mut stage);
    stage.model([too_long(), Line::says("S"), Line::says("嗯。")]);
    stage.say("再说");
    // 四组去掉 20%，向上取整是一组：切在第二组（6 号）前面。
    assert_eq!(summaries(&stage), [(14, None), (14, Some(5))]);
    let (_, truncated) = &stage.requests()[4];
    assert_eq!(
        listed_request(truncated),
        format!(
            "truncated after 5\n{}summarize\n",
            listing(&stage.log()[5..14])
        )
    );
    // 两次摘要请求都记了，前一次是超长。
    let calls: Vec<_> = stage
        .model_calls()
        .into_iter()
        .filter(|called| called.compaction.is_some())
        .map(|called| (called.seen.get(), called.result.clone()))
        .collect();
    assert_eq!(
        calls,
        [(14, CallResult::Error), (14, CallResult::Ok)],
        "同一次压缩的两次摘要请求"
    );
    // 截过的压缩，最后写摘要没看到的那一段：从头到截到的那一条。
    let compacted = last_compaction(&stage).unwrap();
    assert_eq!(compacted.upto.get(), 14);
    assert_eq!(compacted.notes, "<retrieve 14/><uncovered 1-5/>");
}

#[test]
fn a_fifth_is_rounded_up() {
    // 六组（2、6、9、12、15、18 开头，到 20）：20% 是 1.2 组，向上取整去掉两组，切在 8 后面。
    let mut stage = shortening(Some(SHORTEN));
    groups(&mut stage, 4);
    stage.model([too_long(), Line::says("S"), Line::says("嗯。")]);
    stage.say("再说");
    assert_eq!(summaries(&stage), [(20, None), (20, Some(8))]);
}

#[test]
fn with_a_count_it_drops_just_enough_groups_to_cover_it() {
    let mut stage = shortening(Some(SHORTEN));
    four_groups(&mut stage);
    // 超的比第一组多一个：第一组盖不住，去掉两组，切在 8 后面。
    let excess = size(&stage, 1, 5) + 1;
    stage.model([
        too_long().exceeds(excess),
        Line::says("S"),
        Line::says("嗯。"),
    ]);
    stage.say("再说");
    assert_eq!(summaries(&stage), [(14, None), (14, Some(8))]);
    // 正好盖住第一组的，只去掉一组。
    let mut stage = shortening(Some(SHORTEN));
    four_groups(&mut stage);
    let excess = size(&stage, 1, 5);
    stage.model([
        too_long().exceeds(excess),
        Line::says("S"),
        Line::says("嗯。"),
    ]);
    stage.say("再说");
    assert_eq!(summaries(&stage), [(14, None), (14, Some(5))]);
    // 盖不住的，去到只剩一组。
    let mut stage = shortening(Some(SHORTEN));
    four_groups(&mut stage);
    stage.model([
        too_long().exceeds(100_000),
        Line::says("S"),
        Line::says("嗯。"),
    ]);
    stage.say("再说");
    assert_eq!(summaries(&stage), [(14, None), (14, Some(11))]);
}

#[test]
fn it_tries_three_times_then_the_compaction_fails() {
    let mut stage = shortening(Some(SHORTEN));
    // 五组（2、6、9、12、15 开头，到 17）：截三次，每次一组，还剩两组也不再截。
    groups(&mut stage, 3);
    stage.model([too_long(), too_long(), too_long(), too_long()]);
    stage.say("再说");
    assert_eq!(
        summaries(&stage),
        [(17, None), (17, Some(5)), (17, Some(8)), (17, Some(11))]
    );
    assert_eq!(
        story(&stage).last().map(String::as_str),
        Some(format!("{} turn.ended:error kernel t19", stage.log().len()).as_str()),
        "第三次截过还超长，这一轮出错结束"
    );
    assert!(last_compaction(&stage).is_none());
}

#[test]
fn asked_again_after_an_error_it_keeps_the_cut_and_the_count() {
    let mut stage = shortening(Some(SHORTEN));
    // 五组（到 17）：截过一次以后出错、到点再来，照截过的那一份发，截的次数接着数；截满三次还超长，这次压缩失败
    // （施工 6-6 补）。
    groups(&mut stage, 3);
    stage.model([
        too_long(),
        Line::fails(ErrorClass::Retryable, "503 Service Unavailable"),
        too_long(),
        too_long(),
        too_long(),
    ]);
    stage.say("再说");
    assert_eq!(
        summaries(&stage),
        [
            (17, None),
            (17, Some(5)),
            (17, Some(5)),
            (17, Some(8)),
            (17, Some(11))
        ]
    );
    assert!(
        story(&stage)
            .last()
            .is_some_and(|line| line.contains("turn.ended:error")),
        "截满三次还超长，这一轮出错结束"
    );
    assert!(last_compaction(&stage).is_none());
}

#[test]
fn with_one_group_left_it_cannot_cut_and_fails() {
    let mut stage = shortening(Some(Shorten {
        tries: 5,
        percent: 20,
    }));
    four_groups(&mut stage);
    stage.model([too_long(), too_long(), too_long(), too_long()]);
    stage.say("再说");
    // 截了三次，只剩最后一组；第四次报超长截不动了，照失败算，不再发。
    assert_eq!(summaries(&stage).len(), 4);
    assert_eq!(
        story(&stage).last().map(String::as_str),
        Some(format!("{} turn.ended:error kernel t16", stage.log().len()).as_str())
    );
}

#[test]
fn without_the_numbers_in_the_snapshot_an_overflow_just_fails() {
    let mut stage = shortening(None);
    four_groups(&mut stage);
    stage.model([too_long()]);
    stage.say("再说");
    assert_eq!(summaries(&stage), [(14, None)]);
    assert_eq!(
        story(&stage).last().map(String::as_str),
        Some(format!("{} turn.ended:error kernel t16", stage.log().len()).as_str())
    );
}

#[test]
fn an_earlier_checkpoint_stays_and_the_uncovered_part_starts_after_it() {
    let mut stage = shortening(Some(SHORTEN));
    four_groups(&mut stage);
    // 先压一次：检查点替代到 14。第二轮问一句，第三轮再压，报超长截短。
    stage.model([Line::says("S1"), Line::says("嗯。")]);
    stage.say("再说");
    let first = last_compaction(&stage).unwrap();
    assert_eq!(first.upto.get(), 14);
    stage.model([Line::calls("", &[("read", "{}")]), Line::says("好。")]);
    stage.tools([Play::Done("c.rs".to_string())]);
    stage.limits(Some(100_000), None);
    stage.say("又看看");
    stage.limits(Some(120), None);
    stage.model([too_long(), Line::says("S2"), Line::says("嗯。")]);
    stage.say("还有");
    let second = last_compaction(&stage).unwrap();
    let (seen, cut) = *summaries(&stage).last().unwrap();
    assert_eq!(seen, second.upto.get());
    let cut = cut.expect("截过");
    assert!(
        second.notes.ends_with(&format!("<uncovered 15-{cut}/>")),
        "摘要没看到的从上一个检查点后面第一条算起：{}",
        second.notes
    );
}
