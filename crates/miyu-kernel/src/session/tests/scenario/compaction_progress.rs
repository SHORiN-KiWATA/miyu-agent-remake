//! 场景：压缩的进度（`docs/blueprint/compaction.md` 第三条第 8 条，施工 6-3 下；6-11 再补带上 `trigger`）。数同
//! `compaction.rs`：压缩线 100，替身的回复报 110。

use super::compaction::{after_one_turn, compacting, line};
use super::*;
use crate::event::{CompactTrigger, CompactionProgress, TransientBody};

/// 推过的进度，照先后。
fn progress(stage: &Stage) -> Vec<&CompactionProgress> {
    stage
        .transients()
        .iter()
        .filter_map(|transient| match &transient.body {
            TransientBody::CompactionProgress(progress) => Some(progress),
            _ => None,
        })
        .collect()
}

#[test]
fn progress_is_pushed_instead_of_the_summary_text() {
    let mut stage = compacting(None);
    after_one_turn(&mut stage);
    stage.model([Line::says("摘要").thinking("先想想"), Line::says("嗯。")]);
    stage.say("再说一句");
    // 替身一个字一段增量：思考不数，正文两段两个字。
    let pushed: Vec<(u64, u64, u64)> = progress(&stage)
        .iter()
        .map(|p| (p.seen.get(), p.written, p.expected))
        .collect();
    // 发出去时先一条 0 字的（施工 6-3 下）。
    assert_eq!(pushed, [(8, 0, 20_000), (8, 2, 20_000)]);
    assert!(
        progress(&stage)
            .iter()
            .all(|p| p.trigger == CompactTrigger::Auto),
        "到线自动压的，进度带 auto（施工 6-11 再补）"
    );
    let deltas = stage
        .transients()
        .iter()
        .filter(|transient| {
            matches!(&transient.body, TransientBody::ModelDelta(delta) if delta.seen == seq(8))
        })
        .count();
    assert_eq!(deltas, 0, "摘要请求不推增量");
}

#[test]
fn the_expected_length_is_the_usage_kept_between_the_bounds() {
    // 用量是报的加上锚以后新增的几条：五万出头的照原样，九万的夹到八万。
    for (reported, expected) in [(90_000, 80_000..=80_000), (50_000, 50_001..=50_100)] {
        let mut stage = compacting(None);
        stage.model([Line::says("好。").reports(reported)]);
        stage.say("hi");
        line(&mut stage, 100);
        stage.model([Line::says("S1"), Line::says("嗯。")]);
        stage.say("再说一句");
        let p = progress(&stage);
        assert!(!p.is_empty());
        assert!(
            p.iter().all(|p| expected.contains(&p.expected)),
            "报 {reported}：{:?}",
            p.iter().map(|p| p.expected).collect::<Vec<_>>()
        );
    }
}
