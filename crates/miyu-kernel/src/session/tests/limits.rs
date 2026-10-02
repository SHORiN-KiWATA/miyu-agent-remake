//! 给头看的限额（施工 6-3 补，`docs/blueprint/kernel/session.md` 的 `context_limits()`）：窗口照交来的，压缩线和内核判到线
//! 用的是同一条（`compaction.md` 第二条第 2 条）。
//!
//! 策略里压缩的数照出厂的：输出预留的上限 20000、余量 13000。

use super::*;
use crate::estimate::Flat;
use crate::session::{Compaction, ContextLimits};

/// 出厂的两个数；尾巴、图片的数这里用不上。
const COMPACTION: Compaction = Compaction {
    reserve_cap: 20_000,
    margin: 13_000,
    tail: 16_000,
    price: Flat {
        image: 2_000,
        file: 2_000,
    },
    rebuild: None,
    pause: None,
    shorten: None,
    isolate: false,
};

/// 策略里有压缩的会话。
fn compacting() -> Session {
    let mut policy = policy();
    policy.compaction = Some(COMPACTION);
    session_with(policy)
}

/// 交一份限额：替身的模型，窗口、最大输出照给的。
fn hand(session: &mut Session, window: Option<u64>, max_output: Option<u64>) {
    let actions = session.handle(Input::Limits(Limits {
        model: crate::testkit::model(),
        window,
        max_output,
        images: None,
        blind: false,
    }));
    assert!(actions.is_empty(), "交限额什么动作都不出：{actions:?}");
}

fn limits(window: Option<u64>, compaction_line: Option<u64>) -> ContextLimits {
    ContextLimits {
        window,
        compaction_line,
    }
}

#[test]
fn before_any_limits_both_are_absent() {
    assert_eq!(compacting().context_limits(), limits(None, None));
}

#[test]
fn the_line_reserves_the_smaller_of_max_output_and_the_cap() {
    let mut session = compacting();
    hand(&mut session, Some(1_000_000), Some(393_216));
    assert_eq!(
        session.context_limits(),
        limits(Some(1_000_000), Some(967_000)),
        "最大输出比上限大：预留 20000"
    );
    hand(&mut session, Some(128_000), Some(8_000));
    assert_eq!(
        session.context_limits(),
        limits(Some(128_000), Some(107_000)),
        "最大输出比上限小：预留 8000；再交一次照新的"
    );
    hand(&mut session, Some(60_000), None);
    assert_eq!(
        session.context_limits(),
        limits(Some(60_000), Some(27_000)),
        "没报最大输出：预留照上限"
    );
}

#[test]
fn too_small_a_window_has_no_line() {
    let mut session = compacting();
    hand(&mut session, Some(33_000), None);
    assert_eq!(session.context_limits(), limits(Some(33_000), None));
    hand(&mut session, Some(33_001), None);
    assert_eq!(session.context_limits(), limits(Some(33_001), Some(1)));
}

#[test]
fn without_a_window_there_is_no_line_either() {
    let mut session = compacting();
    hand(&mut session, None, Some(8_000));
    assert_eq!(session.context_limits(), limits(None, None));
}

#[test]
fn without_compaction_in_the_policy_only_the_window() {
    let mut session = session();
    hand(&mut session, Some(1_000_000), Some(393_216));
    assert_eq!(session.context_limits(), limits(Some(1_000_000), None));
}

/// 这时的上下文用量（施工 8-15）：没交过限额的、策略里没有压缩的算不了；还没请求过的照本地估算；答完了照供应商报的那一次
/// 起算（锚盖住了整份请求和回复），加上后来进来的那一点。
#[test]
fn the_context_used_follows_the_compaction_estimate() {
    assert_eq!(compacting().context_used(), None, "没交过限额");
    let mut plain = session();
    hand(&mut plain, Some(1_000_000), None);
    assert_eq!(plain.context_used(), None, "策略里没有压缩");
    let mut session = compacting();
    hand(&mut session, Some(1_000_000), None);
    let empty = session.context_used().expect("交了限额就算得出");
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    let asked = session.context_used().expect("算得出");
    assert!(asked > empty, "多了人说的一句：{asked} > {empty}");
    let reply = super::executor::answer(&mut session, 5, "好");
    assert!(!reply.is_empty());
    let reported = super::executor::usage();
    let total = reported.uncached + reported.cache_read + reported.cache_write + reported.output;
    // 锚是供应商报的这一次，锚以后新进有效历史的（这一轮结束时记的几样）照本地估算加上去。
    let used = session.context_used().expect("算得出");
    assert!(
        (total..total + 50).contains(&used),
        "照供应商报的那一次起算：{used}，报的 {total}"
    );
}
