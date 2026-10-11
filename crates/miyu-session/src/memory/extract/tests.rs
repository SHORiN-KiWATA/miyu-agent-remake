//! 抽取怎么拼这一段（`plan`）、一个会话的抽取状态（`Extractor`）：几轮怎么接、够不够、她记过的跳过、请求的字一个字一个字地
//! 对、放不下的分几次、一轮自己就放不下的截了中间也抽；闹钟作废、在路上的只有一个、同一段失败三次才放过。

use std::path::Path;

use miyu_kernel::time::Timestamp;

use super::*;

fn texts() -> ExtractTexts {
    ExtractTexts::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
        .expect("读得到")
}

fn seq(n: u64) -> Seq {
    Seq::new(n).expect("从 1 起")
}

fn at() -> Timestamp {
    Timestamp::parse("2026-10-09T03:00:00.000Z").expect("合写法")
}

/// 一轮：人在第 `said` 条说 `asked`，她在第 `said + 2` 条答 `answer`（回合从第 `said + 1` 条开始）。
fn exchange(said: u64, asked: &str, answer: &str) -> [Spoken; 2] {
    [
        Spoken {
            seq: seq(said),
            turn: None,
            at: at(),
            assistant: false,
            text: asked.to_string(),
        },
        Spoken {
            seq: seq(said + 2),
            turn: Some(TurnId::new(seq(said + 1))),
            at: at(),
            assistant: true,
            text: answer.to_string(),
        },
    ]
}

fn ask(plan: Plan) -> (Seq, String, BTreeSet<u64>, bool) {
    match plan {
        Plan::Ask {
            upto,
            text,
            turns,
            more,
        } => (upto, text, turns, more),
        other => panic!("要发：{other:?}"),
    }
}

#[test]
fn two_turns_are_written_after_the_instruction_turn_by_turn() {
    let spoken: Vec<Spoken> = [
        exchange(2, "我养了一只猫", "好的。"),
        exchange(6, "它叫团子", "记住了。"),
    ]
    .concat();
    let (upto, text, turns, more) =
        ask(plan(&spoken, &BTreeSet::new(), 2, UtcOffset::UTC, &texts()));
    assert_eq!(upto, seq(8));
    assert_eq!(turns, [3, 7].into_iter().collect());
    assert!(!more);
    let instruction = say(&texts().instruction, &[]);
    assert_eq!(
        text,
        format!(
            "{}\n<turn number=\"3\" date=\"2026-10-09\">\nUser: 我养了一只猫\nAssistant: 好的。\n</turn>\n\n<turn number=\"7\" date=\"2026-10-09\">\nUser: 它叫团子\nAssistant: 记住了。\n</turn>",
            instruction.trim_end()
        )
    );
}

#[test]
fn too_few_answered_turns_wait_and_an_unanswered_line_is_left_for_later() {
    let one = exchange(2, "一", "答一");
    assert_eq!(
        plan(&one, &BTreeSet::new(), 2, UtcOffset::UTC, &texts()),
        Plan::Wait
    );
    assert_eq!(
        plan(&[], &BTreeSet::new(), 1, UtcOffset::UTC, &texts()),
        Plan::Wait
    );
    let mut spoken = [exchange(2, "一", "答一"), exchange(6, "二", "答二")].concat();
    spoken.push(Spoken {
        seq: seq(10),
        turn: None,
        at: at(),
        assistant: false,
        text: "还没答的".to_string(),
    });
    let (upto, text, _, _) = ask(plan(&spoken, &BTreeSet::new(), 2, UtcOffset::UTC, &texts()));
    assert_eq!(upto, seq(8), "抽到答了的最后一轮");
    assert!(!text.contains("还没答的"));
}

#[test]
fn a_slice_where_she_remembered_is_skipped_whole() {
    let spoken = [exchange(2, "一", "答一"), exchange(6, "二", "答二")].concat();
    for name in [REMEMBER, FORGET] {
        let called: BTreeSet<String> = [name.to_string(), "read".to_string()].into_iter().collect();
        assert_eq!(
            plan(&spoken, &called, 2, UtcOffset::UTC, &texts()),
            Plan::Skip { upto: seq(8) },
            "{name}"
        );
    }
    let called: BTreeSet<String> = ["read".to_string()].into_iter().collect();
    assert!(matches!(
        plan(&spoken, &called, 2, UtcOffset::UTC, &texts()),
        Plan::Ask { .. }
    ));
}

#[test]
fn what_does_not_fit_waits_for_the_next_go_and_one_huge_turn_is_excerpted() {
    let long = "长".repeat(ROOM / 3 / 3);
    let spoken = [
        exchange(2, &long, "答一"),
        exchange(6, &long, "答二"),
        exchange(10, &long, "答三"),
    ]
    .concat();
    let (upto, text, turns, more) =
        ask(plan(&spoken, &BTreeSet::new(), 2, UtcOffset::UTC, &texts()));
    assert_eq!(
        (upto, turns.len(), more),
        (seq(8), 2, true),
        "取最老的两轮，剩一轮下次"
    );
    assert!(text.len() <= ROOM + say(&texts().instruction, &[]).len() + 1);
    let huge = "大".repeat(ROOM);
    let spoken = [exchange(2, &huge, "答一"), exchange(6, "二", "答二")].concat();
    let (upto, text, turns, more) =
        ask(plan(&spoken, &BTreeSet::new(), 2, UtcOffset::UTC, &texts()));
    assert_eq!(
        (upto, turns.len(), more),
        (seq(4), 1, true),
        "截了中间也抽，往前走"
    );
    assert!(text.contains("[... excerpted ...]"));
    assert!(text.contains("答一"), "尾巴留着");
    assert!(text.len() < ROOM + say(&texts().instruction, &[]).len() + 64);
}

#[test]
fn an_alarm_is_void_once_another_is_set_or_a_turn_starts() {
    let extractor = Extractor::default();
    let first = extractor.arm();
    assert!(extractor.due(first));
    let second = extractor.arm();
    assert!(!extractor.due(first), "上了新的，旧的作废");
    extractor.cancel();
    assert!(!extractor.due(second), "忙起来了");
    let third = extractor.arm();
    assert!(extractor.start(third));
    assert!(!extractor.due(third), "在路上的只有一个");
    assert!(!extractor.start(third));
    assert_eq!(extractor.finish(seq(1), true), 0);
    assert!(extractor.due(third));
    extractor.start(third);
    extractor.abandon();
    assert!(extractor.due(third), "读不成的放下，不算失败");
}

#[test]
fn the_same_slice_failing_three_times_is_let_go() {
    let extractor = Extractor::default();
    let failed = |after: u64| {
        let generation = extractor.arm();
        assert!(extractor.start(generation));
        extractor.finish(seq(after), false)
    };
    assert_eq!(failed(5), 1);
    assert_eq!(failed(5), 2);
    assert_eq!(failed(9), 1, "换了一段重新数");
    assert_eq!(failed(9), 2);
    assert_eq!(failed(9), 3);
    assert_eq!(failed(9), 1, "放过以后重新数");
}

#[test]
fn an_alarm_is_pending_until_it_starts_or_is_let_go() {
    let extractor = Extractor::default();
    assert!(!extractor.pending(), "没上过闹钟");
    let first = extractor.arm();
    assert!(extractor.pending(), "上了，还没响");
    extractor.disarm(first);
    assert!(!extractor.pending(), "响了、不抽，放下了");
    let second = extractor.arm();
    extractor.disarm(first);
    assert!(extractor.pending(), "作废的那一个放下不算数");
    assert!(extractor.start(second));
    assert!(extractor.pending(), "在路上");
    extractor.finish(seq(1), true);
    assert!(!extractor.pending(), "抽完了");
    extractor.arm();
    extractor.cancel();
    assert!(!extractor.pending(), "忙起来撤掉了");
    let third = extractor.arm();
    extractor.start(third);
    extractor.abandon();
    assert!(!extractor.pending(), "读不成的放下了");
}
