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
    assert_eq!(
        shown(&t),
        [(Kind::Note, "· 上下文已压缩：812.3k → 31k token".to_string())]
    );
    assert_eq!(t.entries[0].progress, None, "压好了不转、不画进度条");
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
    assert!(t.waiting(), "压完了还没出字：接着等第一个字");
}
