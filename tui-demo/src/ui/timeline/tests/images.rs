//! 结果里带图的步（蓝图 `tui.md`「时间线」第 8 条）：图照内容的哈希读回来存成文件，读回来了交给画图的那一套；没读回来的
//! 记进单子（和链接卡片的图同一份账）。

use std::path::PathBuf;
use std::time::Instant;

use serde_json::json;

use ratatui_image::picker::{Picker, ProtocolType};

use super::{rows, segment, thought};
use crate::core::ToolStatus;
use crate::figures::{Figures, Graphics};
use crate::markdown::FigureKind;
use crate::transcript::{Step, StepKind, ToolState};
use crate::ui::rows::{Row, Target};
use crate::ui::test_support::Fixture;
use crate::ui::timeline::step::figures;

fn read_image() -> Step {
    let mut step = Step::new(StepKind::Tool {
        name: "read".into(),
        args: String::new(),
        parsed: json!({"path": "/tmp/cat.png"}),
        state: ToolState::Done(ToolStatus::Ok),
        output: "cat.png".into(),
        said: None,
    });
    step.started = Instant::now();
    step.images = vec!["sha256:aa".into(), "sha256:bb".into()];
    step
}

#[test]
fn images_already_read_back_become_figures_and_the_rest_are_asked_for() {
    let f = Fixture::new();
    f.cards.borrow_mut().saved(
        "sha256:aa".into(),
        Some(PathBuf::from("/cache/blobs/aa.png")),
    );
    let got = figures(&read_image(), &f.ctx());
    assert_eq!(got.len(), 1, "读回来的那张才画");
    assert_eq!(
        got[0].kind,
        FigureKind::Preview,
        "工具结果里的图照预览的大小画（2026-10-10）"
    );
    assert_eq!(got[0].source, "/cache/blobs/aa.png");
    let (_, blobs) = f.cards.borrow_mut().take();
    assert_eq!(blobs, ["sha256:bb"], "没读回来的记进单子");
}

/// 能画图的终端：图先排着队，占一行「图片加载中」。
fn with_pictures(f: &Fixture) {
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    *f.ctx().figures.borrow_mut() =
        Figures::start(Some(Graphics { picker }), &f.config.figures, None, |_| true);
    f.cards.borrow_mut().saved(
        "sha256:aa".into(),
        Some(PathBuf::from("/cache/blobs/aa.png")),
    );
}

/// 只读了一张图、没有字的那种结果。
fn bare_read() -> Step {
    let mut step = read_image();
    if let StepKind::Tool { output, .. } = &mut step.kind {
        output.clear();
    }
    step.open = Some(true);
    step
}

/// 点开的那一块：空一行、图、空一行，整块铺底色；不先空两行再把图挂在块外面（2026-10-11 项目主人，#78）。
fn assert_inside(block: &[Row]) {
    let at = block
        .iter()
        .position(|r| r.figure_pending)
        .expect("点开了图也在");
    let texts: Vec<String> = block.iter().map(|r| r.line.to_string()).collect();
    assert!(
        block[at - 1].line.to_string().trim().is_empty() && at >= 1,
        "图上面空一行：{texts:?}"
    );
    assert!(
        at < 2 || !block[at - 2].line.to_string().trim().is_empty(),
        "没字的结果不留两行空的：{texts:?}"
    );
    let last = block.last().unwrap();
    assert!(
        at + 1 < block.len() && last.line.to_string().trim().is_empty(),
        "图在块里，块末尾空一行收住：{texts:?}"
    );
    assert!(
        block[at - 1..].iter().all(|r| r.shade),
        "图和收尾的空行都铺底色：{texts:?}"
    );
}

#[test]
fn an_opened_step_keeps_its_picture_inside_the_shaded_block() {
    let f = Fixture::new();
    with_pictures(&f);
    let t0 = std::time::Instant::now();
    let seg = segment(vec![thought(t0, 0), bare_read()], Some(true));
    let all = rows(0, &seg, &f.ctx());
    let mine: Vec<Row> = all
        .into_iter()
        .filter(|r| r.target == Some(Target::Step(0, 1)))
        .collect();
    assert_inside(&mine);
}

#[test]
fn a_segment_of_one_picture_read_opens_with_the_picture() {
    let f = Fixture::new();
    with_pictures(&f);
    let seg = segment(vec![bare_read()], Some(true));
    let all = rows(0, &seg, &f.ctx());
    assert_inside(&all);
}
