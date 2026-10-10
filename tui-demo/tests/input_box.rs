//! 伪终端里的输入框（蓝图 `tui.md`「输入框」）：空着时光标和提示的位置。

mod support;

use std::time::Duration;

use miyu_session::testkit::Script;
use support::Home;

#[test]
fn the_cursor_waits_on_a_blank_cell_before_the_tip() {
    // 「输入框」第 9 条（2026-10-10 项目主人：光标压在提示的第一个字上，「不是光标而是选中了输入框背景 tips 里的文字」）。
    let home = Home::new(Script::new([]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("Tab 切换权限级别");
    tui.pump(Duration::from_millis(300));
    let (row, col) = tui.cursor();
    assert_eq!(tui.cell(row, col).trim(), "", "光标停在空着的一格上");
    assert_eq!(tui.cell(row, col + 1), "T", "提示从下一格写起");
}
