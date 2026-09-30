//! 收尾行和用量（蓝图 `tui.md`「正文」第 4 条）：`✻ 时刻 · 端点/模型 · 用时 · 本轮用量(C命中%)`。

use super::apply;
use crate::core::{EndReason, Push, Usage};
use crate::transcript::{Kind, Transcript};

#[test]
fn a_completed_turn_ends_with_the_done_line() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::Model {
                endpoint: "deepseek".into(),
                model: "deepseek-flash".into(),
            },
            Push::TurnStarted(1, None),
            Push::TurnEnded(crate::core::EndReason::Completed),
        ],
    );
    let done = t.entries.last().unwrap();
    assert_eq!(done.kind, Kind::Done);
    // `HH:MM · 模型 · 用时`，不到一秒的也带一位小数，不写成 0 秒。
    let parts: Vec<&str> = done.text.split(" · ").collect();
    assert_eq!(parts.len(), 3, "{}", done.text);
    assert_eq!(parts[0].len(), "01:43".len());
    assert_eq!(parts[1], "deepseek/deepseek-flash", "端点/模型");
    assert!(
        parts[2].starts_with("0.") && parts[2].ends_with('s'),
        "{}",
        done.text
    );
}

#[test]
fn usage_adds_up_and_context_is_the_last_call() {
    let mut t = Transcript::default();
    let call = |input, output| {
        Push::Usage(Usage {
            uncached: input,
            output,
            ..Usage::default()
        })
    };
    apply(&mut t, vec![call(100, 10), call(200, 20)]);
    assert_eq!((t.total.input(), t.total.output, t.context), (300, 30, 220));
}

#[test]
fn the_done_line_counts_this_turn_and_its_cache_hits() {
    let call = |uncached, cache_read, output| {
        Push::Usage(Usage {
            uncached,
            cache_read,
            cache_write: 0,
            output,
            aux: 0,
        })
    };
    let turn = |n, calls: Vec<Push>| {
        let mut pushes = vec![Push::TurnStarted(n, None)];
        pushes.extend(calls);
        pushes.push(Push::TurnEnded(EndReason::Completed));
        pushes
    };
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![Push::Model {
            endpoint: "deepseek".into(),
            model: "deepseek-flash".into(),
        }],
    );
    // 一轮两次请求：输入 1000 + 1020，输出 50 + 30，一共 2100；命中 1900 ÷ 2020 = 94%。
    apply(
        &mut t,
        turn(1, vec![call(100, 900, 50), call(20, 1000, 30)]),
    );
    let done = &t.entries.last().unwrap().text;
    assert!(done.ends_with(" · 2.1k(C94%)"), "{done}");
    // 下一轮重新算，不带上一轮的。
    apply(&mut t, turn(2, vec![call(0, 1000, 0)]));
    let done = &t.entries.last().unwrap().text;
    assert!(done.ends_with(" · 1k(C100%)"), "{done}");
    // 一次模型都没调成的，不写这一截。
    apply(&mut t, turn(3, Vec::new()));
    let done = &t.entries.last().unwrap().text;
    assert_eq!(done.split(" · ").count(), 3, "{done}");
}

#[test]
fn the_done_line_keeps_the_level_its_turn_ran_with() {
    use crate::config::Config;
    use crate::core::Level;
    let order = Config::builtin().unwrap().layout.level_cycle;
    let mut t = Transcript::default();
    let next = t.next_level(&order); // 工作区 → 开放权限
    apply(&mut t, vec![super::policy(next)]);
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::TurnEnded(EndReason::Completed),
        ],
    );
    assert_eq!(t.entries.last().unwrap().level, Some(Level::Full));
    // 之后换了级别，已经写出来的不跟着变；被打断的那一行也记着。
    let next = t.next_level(&order); // → 只读
    apply(&mut t, vec![super::policy(next)]);
    assert_eq!(t.entries.last().unwrap().level, Some(Level::Full));
    apply(
        &mut t,
        vec![
            Push::TurnStarted(2, None),
            Push::TurnEnded(EndReason::Interrupted),
        ],
    );
    assert_eq!(t.entries.last().unwrap().level, Some(Level::ReadOnly));
}

#[test]
fn the_core_limits_are_kept_for_the_sidebar_and_footer() {
    use crate::config::Config;
    use crate::core::{Limits, Update};
    let mut t = Transcript::default();
    let texts = Config::builtin().unwrap().text;
    let limits = Limits {
        window: Some(300_000),
        compaction_line: Some(267_000),
    };
    t.update(Update::Limits(limits), &texts);
    assert_eq!(t.limits, limits);
}
