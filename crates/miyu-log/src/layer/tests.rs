//! 事件写成一行：级别怎么筛、会话编号从哪来、别人家的最多记到 WARN。

use std::sync::Arc;

use tracing_subscriber::filter::LevelFilter;

use crate::layer::Memory;
use crate::subscriber;

/// 定住的时刻。
fn clock() -> String {
    "2026-09-27 21:03:15.284".to_string()
}

/// 照 `filter` 装一个写进内存的订阅者（筛法和 [`subscriber`] 一样），跑 `body`，交回写下的行。
fn capture(filter: LevelFilter, body: impl FnOnce()) -> Vec<String> {
    let memory = Memory::new();
    let sink: Arc<dyn crate::Sink> = memory.clone();
    tracing::subscriber::with_default(crate::with_clock(sink, filter, clock), body);
    memory.lines()
}

#[test]
fn an_event_is_one_line_in_the_drawing_s_shape() {
    let lines = capture(LevelFilter::INFO, || {
        tracing::info!(target: "miyu::core", version = "0.0.0", root = "~/.miyu", "started");
        tracing::warn!(target: "miyu::session", session = "0199d1e6", attempt = 1, "rate limited (429)");
    });
    assert_eq!(
        lines,
        [
            "2026-09-27 21:03:15.284 INFO  core     started version=0.0.0 root=~/.miyu",
            "2026-09-27 21:03:15.284 WARN  session  0199d1e6 rate limited (429) attempt=1",
        ]
    );
}

#[test]
fn the_session_comes_from_the_span_around_it_at_any_level() {
    // 会话的 span 开在 ERROR 级：span 也照级别筛，开在 INFO 的话，调到 WARN 它就被筛掉了，底下的行
    // 就没了会话编号。
    let run = || {
        let span = tracing::error_span!(target: "miyu::session", "session", session = "0199d1e6");
        let _entered = span.enter();
        tracing::debug!(target: "miyu::http", host = "api.deepseek.com", bytes = 2396, "sent");
        tracing::warn!(target: "miyu::session", "rate limited");
    };
    assert_eq!(
        capture(LevelFilter::DEBUG, run),
        [
            "2026-09-27 21:03:15.284 DEBUG http     0199d1e6 sent host=api.deepseek.com bytes=2396",
            "2026-09-27 21:03:15.284 WARN  session  0199d1e6 rate limited",
        ]
    );
    assert_eq!(
        capture(LevelFilter::WARN, run),
        ["2026-09-27 21:03:15.284 WARN  session  0199d1e6 rate limited"]
    );
}

#[test]
fn the_nearest_span_s_session_wins_and_one_recorded_later_counts() {
    let lines = capture(LevelFilter::INFO, || {
        let outer = tracing::error_span!(target: "miyu::session", "session", session = "outer");
        let _outer = outer.enter();
        let inner = tracing::error_span!(
            target: "miyu::session",
            "session",
            session = tracing::field::Empty
        );
        let _inner = inner.enter();
        inner.record("session", "inner");
        tracing::info!(target: "miyu::core", "here");
    });
    assert_eq!(lines, ["2026-09-27 21:03:15.284 INFO  core     inner here"]);
}

#[test]
fn levels_below_the_filter_and_others_below_warn_are_not_written() {
    let lines = capture(LevelFilter::INFO, || {
        tracing::debug!(target: "miyu::http", "hidden at info");
        tracing::info!(target: "hyper::proto", "hidden: not ours");
        tracing::warn!(target: "hyper::proto", "shown: a warning from them");
        tracing::info!(target: "miyu::core", "shown");
    });
    assert_eq!(
        lines,
        [
            "2026-09-27 21:03:15.284 WARN  hyper::proto shown: a warning from them",
            "2026-09-27 21:03:15.284 INFO  core     shown",
        ]
    );
}

#[test]
fn the_real_subscriber_filters_the_same_way() {
    let memory = Memory::new();
    let sink: Arc<dyn crate::Sink> = memory.clone();
    tracing::subscriber::with_default(subscriber(sink, LevelFilter::WARN), || {
        tracing::info!(target: "miyu::core", "hidden at warn");
        tracing::error!(target: "miyu::core", "shown");
    });
    let lines = memory.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].ends_with("ERROR core     shown"), "{}", lines[0]);
}

#[test]
fn off_writes_nothing_and_a_stricter_filter_holds_for_them_too() {
    let lines = capture(LevelFilter::OFF, || {
        tracing::error!(target: "miyu::core", "hidden: off");
        tracing::warn!(target: "hyper::proto", "hidden: off for them too");
    });
    assert!(lines.is_empty(), "{lines:?}");
    let lines = capture(LevelFilter::ERROR, || {
        tracing::warn!(target: "hyper::proto", "hidden: stricter than warn");
        tracing::error!(target: "hyper::proto", "shown");
    });
    assert_eq!(lines, ["2026-09-27 21:03:15.284 ERROR hyper::proto shown"]);
}
