//! 收起那一行跟着界面语言（蓝图 `tui.md`「时间线」第 17 条，2026-10-01 项目主人定）：自动时英文，手动选了哪种照哪种。

use std::time::Instant;

use super::{command, rows, segment, text, thought};
use crate::config::Config;
use crate::core::ToolStatus;
use crate::language::LanguageTable;
use crate::ui::test_support::Fixture;

/// 想过一次、跑了两条命令的一段，照 `code` 那种语言、自动还是手动，收起时那一行。
fn folded(code: &str, auto: bool) -> Vec<String> {
    let table = LanguageTable::builtin().unwrap();
    let mut f = Fixture::new();
    f.config = Config::load(&table.find(code).unwrap(), auto).unwrap();
    let t0 = Instant::now();
    let seg = segment(
        vec![
            thought(t0, 0),
            command(t0, 1, ToolStatus::Ok),
            command(t0, 2, ToolStatus::Ok),
        ],
        None,
    );
    text(&rows(0, &seg, &f.ctx()))
}

#[test]
fn the_folded_line_follows_a_chosen_language_but_stays_english_on_auto() {
    let english = ["  Ran 2 commands · 1 thought · 3s"];
    assert_eq!(folded("zh", true), english, "自动：英文，和正文分开");
    assert_eq!(folded("ja", true), english);
    assert_eq!(folded("zh", false), ["  执行了 2 条命令 · 1 次思考 · 3s"]);
    assert_eq!(
        folded("ja", false),
        ["  コマンドを 2 件実行 · 思考 1 回 · 3s"]
    );
    assert_eq!(folded("en", false), english);
}
