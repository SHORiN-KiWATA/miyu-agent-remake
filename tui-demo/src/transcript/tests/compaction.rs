//! 压缩那几行（蓝图 `tui.md`「正文」第 9 条，照 `miyu ask`）：进度原地刷新，压好了、失败了换成结果，暂停了红字，
//! 被打断的去掉；出错那一行的分类写成人话。

use super::super::{Kind, Transcript};
use super::apply;
use crate::core::{Compaction, EndReason, Push};

fn shown(t: &Transcript) -> Vec<(Kind, String)> {
    t.entries
        .iter()
        .filter(|e| !e.hidden)
        .map(|e| (e.kind.clone(), e.text.clone()))
        .collect()
}

fn progress(written: u64) -> Push {
    Push::Compaction(Compaction::Progress {
        written,
        expected: Some(20000),
    })
}

#[test]
fn progress_updates_one_line_then_becomes_the_result() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![Push::TurnStarted(1, None), progress(0), progress(3120)],
    );
    assert_eq!(
        shown(&t),
        [(Kind::Note, "正在压缩上下文 3,120".to_string())],
        "进度原地刷新，只有一行；流光、点画的时候加"
    );
    let progress = t.entries[0].progress.as_ref().unwrap();
    assert_eq!(
        (progress.written, progress.expected, progress.lit),
        (3120, Some(20000), 0),
        "进度条照它画，亮到哪由每一帧追"
    );
    let done = Compaction::Done {
        before: 812_300,
        after: 31_000,
    };
    apply(&mut t, vec![Push::Compaction(done)]);
    assert!(t.entries[0].progress.is_some(), "压好了先走满进度条");
    frames(&mut t, std::time::Instant::now(), 20);
    assert_eq!(
        shown(&t),
        [(Kind::Note, "上下文已压缩：812.3k → 31k token".to_string())]
    );
    assert_eq!(t.entries[0].progress, None, "走满以后不转、不画进度条");
    assert_eq!(t.entries[0].mark.as_deref(), Some("● "), "前面绿点");
}

#[test]
fn a_failed_summary_is_red_with_its_reason_and_not_the_turns_error() {
    let mut t = Transcript::default();
    let failed = |class: &str, message: &str| {
        Push::Compaction(Compaction::Failed {
            class: class.into(),
            message: message.into(),
        })
    };
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            progress(0),
            failed("rate_limited", "429"),
        ],
    );
    assert_eq!(
        shown(&t),
        [(Kind::Error, "· 压缩失败：被限速了".to_string())]
    );
    apply(
        &mut t,
        vec![
            progress(0),
            failed("bad_summary", "the summary called a tool"),
        ],
    );
    assert_eq!(shown(&t)[1].1, "· 压缩失败：摘要请求里调了工具");
    apply(&mut t, vec![Push::TurnEnded(EndReason::Completed)]);
    assert!(
        !shown(&t).iter().any(|(_, text)| text.starts_with("出错了")),
        "摘要请求出错不算这一轮出错"
    );
}

#[test]
fn an_interrupted_compaction_leaves_no_line() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            progress(500),
            Push::TurnEnded(EndReason::Interrupted),
        ],
    );
    assert!(
        !shown(&t).iter().any(|(_, text)| text.contains("正在压缩")),
        "还在压的那一行去掉，收尾会说：{:?}",
        shown(&t)
    );
}

#[test]
fn a_pause_is_one_red_line_by_its_reason() {
    let paused = |reason: &str, failures: Option<u64>, entry: Option<u64>| {
        let mut t = Transcript::default();
        let push = Compaction::Paused {
            reason: reason.into(),
            failures,
            entry,
        };
        apply(&mut t, vec![Push::Compaction(push)]);
        shown(&t)
    };
    assert_eq!(
        paused("failures", Some(3), None),
        [(
            Kind::Error,
            "· 自动压缩连续失败 3 次，已暂停：可以手动压缩、换一个模型，或者开新会话".to_string()
        )]
    );
    assert_eq!(
        paused("too_large", None, Some(1234))[0].1,
        "· 第 1234 条内容太大，压完很快又满了，自动压缩已暂停"
    );
    let other = "· 自动压缩已暂停：可以手动压缩、换一个模型，或者开新会话";
    assert_eq!(paused("too_large", None, None)[0].1, other, "缺了序号");
    assert_eq!(paused("new_reason", None, None)[0].1, other, "认不得的原因");
}

#[test]
fn the_error_line_names_its_class() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::CallFailed {
                class: "compaction_paused".into(),
                message: "the request does not fit".into(),
            },
            Push::TurnEnded(EndReason::Error),
        ],
    );
    assert_eq!(
        shown(&t).last().unwrap().1,
        "出错了：自动压缩暂停着：the request does not fit"
    );
}

#[test]
fn a_summary_that_called_a_tool_tries_again_without_tools_in_grey() {
    // 施工 6-6 下：摘要请求调了工具、改走隔离式，不是失败（`compaction.md` 第三条第 7 条，照 `miyu ask`）。
    let mut t = Transcript::default();
    let isolating = Push::Compaction(Compaction::Failed {
        class: "bad_summary".into(),
        message: "the summary called a tool (read); trying again without tools".into(),
    });
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            progress(900),
            isolating,
            progress(0),
            progress(40),
        ],
    );
    assert_eq!(
        shown(&t),
        [
            (
                Kind::Note,
                "· 摘要请求里调了工具，改用不带工具的再压".to_string()
            ),
            (Kind::Note, "正在压缩上下文 40".to_string()),
        ],
        "灰色说一句，这次压缩接着来进度"
    );
}

#[test]
fn while_compacting_only_that_line_spins() {
    // 2026-09-29 实测：压缩那一行在转，正文末尾等第一个字的转圈也在转，两个一起转。
    let mut t = Transcript::default();
    apply(&mut t, vec![Push::TurnStarted(1, None)]);
    assert!(t.waiting());
    apply(&mut t, vec![progress(0)]);
    assert!(!t.waiting(), "压缩那一行自己在转");
    let done = Compaction::Done {
        before: 12_200,
        after: 3_800,
    };
    apply(&mut t, vec![Push::Compaction(done)]);
    assert!(!t.waiting(), "条在走满：还是它在动");
    frames(&mut t, std::time::Instant::now(), 20);
    assert!(t.waiting(), "换成结果了还没出字：接着等第一个字");
}

/// 每一帧追一下进度条：`from` 起走 `frames` 帧，一帧 80 毫秒。交回走到的时刻。
fn frames(t: &mut Transcript, from: std::time::Instant, n: u32) -> std::time::Instant {
    let config = crate::config::Config::builtin().unwrap();
    let mut rng = crate::rng::Rng::new(9);
    let mut now = from;
    for _ in 0..n {
        now += std::time::Duration::from_millis(80);
        t.climb(
            now,
            config.layout.bar.width,
            &config.layout.compaction,
            &mut rng,
        );
    }
    now
}

#[test]
fn when_done_the_bar_fills_up_then_turns_into_the_result() {
    // 2026-09-29 项目主人：条走到一半就一下跳成结果，要先快速走满再跳。
    let config = crate::config::Config::builtin().unwrap();
    let width = config.layout.bar.width;
    let mut t = Transcript::default();
    apply(&mut t, vec![Push::TurnStarted(1, None), progress(0)]);
    let now = std::time::Instant::now();
    let done = Compaction::Done {
        before: 12_300,
        after: 4_000,
    };
    apply(&mut t, vec![progress(8_000), Push::Compaction(done)]);
    let entry = |t: &Transcript| {
        t.entries
            .iter()
            .find(|e| e.progress.is_some() || e.text.contains("压缩"))
            .cloned()
            .unwrap()
    };
    assert!(entry(&t).progress.is_some(), "压好了先不换：条接着走");
    let now = frames(&mut t, now, 2);
    let lit = entry(&t).progress.unwrap().lit;
    assert!(lit > 0 && lit < width, "一格格走，不一下满：{lit}");
    // 走满、停一会儿以后换成结果：绿点，字暗。
    frames(&mut t, now, 20);
    let result = entry(&t);
    assert!(result.progress.is_none());
    assert_eq!(result.text, "上下文已压缩：12.3k → 4k token");
    assert_eq!(result.mark.as_deref(), Some("● "));
}

#[test]
fn a_turn_that_ends_while_filling_keeps_filling_and_is_not_hidden() {
    // 手动压缩那一轮压好就结束（压好和结束同一批到）：条照样走满再换，不当成没压完藏起来。
    let mut t = Transcript::default();
    let done = Compaction::Done {
        before: 12_300,
        after: 4_000,
    };
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            progress(8_000),
            Push::Compaction(done),
            Push::TurnEnded(EndReason::Completed),
        ],
    );
    let line = |t: &Transcript| {
        t.entries
            .iter()
            .find(|e| e.progress.is_some() || e.mark.is_some())
            .cloned()
            .unwrap()
    };
    assert!(!line(&t).hidden);
    assert!(line(&t).progress.is_some(), "这一轮结束了，条还在走满");
    assert!(t.busy(), "走满的这几帧照转圈的节拍重画");
    frames(&mut t, std::time::Instant::now(), 20);
    let result = line(&t);
    assert!(
        result.text.starts_with("上下文已压缩：12.3k → 4k token · "),
        "手动压缩那一轮：用时接在后面：{}",
        result.text
    );
    assert_eq!(result.mark.as_deref(), Some("● "));
    assert!(!t.busy(), "换成结果以后不再重画");
}

#[test]
fn a_manual_compaction_turn_takes_no_words() {
    // 施工 6-8：手动压缩那一轮的 turn.started 没有 trigger，不把你说的话归进来。
    let mut t = Transcript::default();
    t.user("还没开轮的一句".into(), Vec::new());
    apply(&mut t, vec![Push::TurnStarted(3, None)]);
    let words = t
        .entries
        .iter()
        .find(|e| e.text == "还没开轮的一句")
        .unwrap();
    assert_eq!(words.turn, None, "不归到压缩那一轮");
}

#[test]
fn a_manual_compaction_puts_its_time_and_usage_on_the_result_line() {
    // 2026-09-29 项目主人：手动压缩以后不另起收尾行，用时和用量接在结果那一行后面。
    use crate::core::Usage;
    let mut t = Transcript::default();
    let done = Compaction::Done {
        before: 66_300,
        after: 17_100,
    };
    apply(
        &mut t,
        vec![
            Push::TurnStarted(3, None),
            progress(8_000),
            Push::Usage(Usage {
                uncached: 400,
                cache_read: 58_000,
                cache_write: 0,
                output: 0,
            }),
            Push::Compaction(done),
            Push::TurnEnded(EndReason::Completed),
        ],
    );
    frames(&mut t, std::time::Instant::now(), 20);
    assert!(
        t.entries.iter().all(|e| e.kind != Kind::Done),
        "不另起收尾行"
    );
    let result = t.entries.iter().find(|e| e.mark.is_some()).unwrap();
    assert!(
        result
            .text
            .starts_with("上下文已压缩：66.3k → 17.1k token · ")
            && result.text.ends_with(" · 58.4k(C99.3%)"),
        "{}",
        result.text
    );
    // 自动压缩发生在普通的一轮里：收尾行照旧。
    let mut t = Transcript::default();
    t.user("读一下".into(), Vec::new());
    let done = Compaction::Done {
        before: 66_300,
        after: 17_100,
    };
    apply(
        &mut t,
        vec![
            Push::UserMessage(1),
            Push::TurnStarted(4, Some(1)),
            progress(8_000),
            Push::Compaction(done),
            Push::TurnEnded(EndReason::Completed),
        ],
    );
    assert!(
        t.entries.iter().any(|e| e.kind == Kind::Done),
        "普通的一轮照旧有收尾行"
    );
}
