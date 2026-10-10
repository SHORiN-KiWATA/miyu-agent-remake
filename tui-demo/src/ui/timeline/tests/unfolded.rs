//! 不收起时只有一步的一段（蓝图 `tui.md`「时间线」第 15 条，2026-10-11 项目主人报：关了「回复完成后折叠时间线」、没开
//! 「展开思考」，做完的思考却铺开了全文）：照进行中的样子，一行步骤加预览；开了展开、人点开的才铺全文。

use std::time::Instant;

use super::{rows, segment, text, thought};
use crate::ui::test_support::Fixture;

#[test]
fn an_unfolded_lone_thought_keeps_its_running_look_until_someone_opens_it() {
    let mut f = Fixture::new();
    f.config.timeline.fold = false;
    let t0 = Instant::now();
    let seg = segment(vec![thought(t0, 0)], None);
    let got = rows(0, &seg, &f.ctx());
    let think = f.config.icons.think.clone();
    assert_eq!(
        text(&got),
        [format!("  {think} 已思考 · 1.0s"), "  │ 先看看目录".into()],
        "没有收起那一行，照进行中的样子"
    );
    assert!(got.iter().all(|r| !r.shade), "没铺开，不铺底色");
    // 开了「展开思考」：铺开全文。
    f.config.timeline.expand.thought = true;
    let got = rows(0, &seg, &f.ctx());
    assert!(got.iter().any(|r| r.shade), "{:?}", text(&got));
    // 人点开那一步：同样铺开。
    f.config.timeline.expand.thought = false;
    let mut seg = seg;
    seg.steps[0].open = Some(true);
    assert!(rows(0, &seg, &f.ctx()).iter().any(|r| r.shade));
}
