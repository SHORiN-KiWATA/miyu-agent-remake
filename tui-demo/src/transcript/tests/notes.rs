//! 插进正文的信息（后台任务的回报、子代理的回报、回顾、换模型这些旁白）：前面那一段还在进行就先收起，
//! 信息画在它下面，接着的步另起一段（2026-10-02 项目主人报：回报直接插进展开的一段里，连接线断、前面那段一直不收）。

use super::super::{Kind, Segment, Transcript};
use super::apply;
use crate::core::{Block, EndReason, Push};
use crate::transcript::JobMark;

/// 开一轮、跑一条命令：时间线的一段在进行中。
fn started_with_command(t: &mut Transcript) {
    apply(
        t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::ToolCall("shell".into()),
            },
            Push::Delta {
                index: 0,
                text: "{\"command\":\"sleep 3\",\"description\":\"等三秒\"}".into(),
            },
            Push::BlockEnd(0),
        ],
    );
}

/// 正文里每一段（照先后）。
fn segments(t: &Transcript) -> Vec<&Segment> {
    t.entries
        .iter()
        .filter_map(|e| e.segment.as_ref())
        .collect()
}

#[test]
fn a_job_notice_folds_the_live_segment_before_it() {
    let mut t = Transcript::default();
    started_with_command(&mut t);
    t.job(
        JobMark::Done,
        "后台命令完成 · sleep 3 · 3s".into(),
        String::new(),
    );
    apply(
        &mut t,
        vec![
            Push::BlockStart {
                index: 1,
                block: Block::Reasoning,
            },
            Push::Delta {
                index: 1,
                text: "接着来".into(),
            },
            Push::BlockEnd(1),
            Push::TurnEnded(EndReason::Completed),
        ],
    );
    let segs = segments(&t);
    assert_eq!(segs.len(), 2, "回报后面的步另起一段");
    assert!(segs[0].finished, "回报前面那一段先收起");
    assert!(segs[1].finished, "一轮结束照旧收起");
    let job = t.entries.iter().position(|e| e.kind == Kind::Job).unwrap();
    let first = t.entries.iter().position(|e| e.segment.is_some()).unwrap();
    let second = t.entries.iter().rposition(|e| e.segment.is_some()).unwrap();
    assert!(
        first < job && job < second,
        "信息画在收起的那一段和接着的那一段中间"
    );
}

#[test]
fn a_note_folds_the_live_segment_before_it() {
    // 换模型、回顾这类旁白走同一个 `note`，也一样。
    let mut t = Transcript::default();
    started_with_command(&mut t);
    t.note(Kind::Note, "↻ 换了模型".into());
    assert!(segments(&t)[0].finished, "旁白前面那一段先收起");
    assert_eq!(t.entries[1].kind, Kind::Note);
    // 没有在进行的那一段时，旁白照常在末尾，不凭空造段。
    t.note(Kind::Note, "又一句".into());
    assert_eq!(t.entries.len(), 3);
    assert_eq!(segments(&t).len(), 1);
}
