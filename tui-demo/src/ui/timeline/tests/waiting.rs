//! 等她的第一个字时，正文末尾先转着（蓝图 `tui.md`「时间线」第 19 条）。

use super::text;
use crate::config::Config;
use crate::core::{Block, Push, Update};
use crate::transcript::Transcript;
use crate::ui::test_support::{Fixture, fresh_rows};
use crate::ui::timeline::tail_rows;

fn apply(t: &mut Transcript, pushes: Vec<Push>) {
    let texts = Config::builtin().unwrap().text;
    for p in pushes {
        t.update(Update::Push(p), &texts);
    }
}

#[test]
fn the_first_step_lands_on_the_waiting_spinner() {
    let f = Fixture::new();
    let ctx = f.ctx();
    let spinner = &f.config.timeline.spinner[0];
    let mut t = Transcript::default();
    t.user("看看目录".into(), Vec::new());
    apply(&mut t, vec![Push::TurnStarted(1, None)]);
    let mut before = fresh_rows(&t.entries, &ctx);
    before.extend(tail_rows(&ctx));
    let spot = before.len() - 1;
    let shown = text(&before);
    assert_eq!(shown[spot - 1], "", "空一行");
    assert_eq!(shown[spot], spinner.as_str(), "只有转圈");
    apply(
        &mut t,
        vec![Push::BlockStart {
            index: 0,
            block: Block::Reasoning,
        }],
    );
    let after = text(&fresh_rows(&t.entries, &ctx));
    assert_eq!(after[..spot], shown[..spot], "上面的不动");
    assert!(
        after[spot].starts_with(spinner.as_str()) && after[spot].contains("思考中"),
        "同一格接着转：{after:?}"
    );
}
