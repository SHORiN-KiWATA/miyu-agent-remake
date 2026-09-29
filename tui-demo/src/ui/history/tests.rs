//! 输入历史列表排成的行（蓝图 `tui.md`「输入历史列表」）：照后台面板的样子，每条右边写多久以前，
//! 命令名、好几行的、粘贴块、搜到的字各有记号。

use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::{index_at, lines};
use crate::config::Config;
use crate::history::History;
use crate::input::{Draft, Sent};
use crate::theme;

fn sent(text: &str, ago: u64, now: Instant) -> Sent {
    Sent {
        draft: Draft::plain(text),
        at: now - Duration::from_secs(ago),
    }
}

fn plain(rows: &[super::Row]) -> Vec<String> {
    rows.iter()
        .map(|(_, l)| l.to_string().trim_end().to_string())
        .collect()
}

#[test]
fn it_looks_like_the_background_panel_and_clicks_find_their_entry() {
    let _theme = theme::hold();
    let config = Config::builtin().unwrap();
    let now = Instant::now();
    let list = [
        sent("新的", 10, now),
        sent("中间", 300, now),
        sent("早的", 7200, now),
    ];
    let found: Vec<&Sent> = list.iter().collect();
    let rows = lines(&History::default(), &found, 40, &config, now);
    let text = plain(&rows);
    // 标题写在横线上（2026-09-29 项目主人）。
    assert!(text[0].starts_with("── 历史  3 条 ─"), "{text:?}");
    assert!(text[0].ends_with('─'));
    assert_eq!(rows[0].1.width(), 40, "横线铺满");
    assert_eq!(text[1], "");
    assert!(
        text[2].starts_with("  早的") && text[2].ends_with("2 小时前"),
        "{text:?}"
    );
    assert!(text[3].starts_with("  中间") && text[3].ends_with("5 分钟前"));
    assert!(text[4].starts_with("❯ 新的") && text[4].ends_with("刚才"));
    assert_eq!(text[5], "");
    assert_eq!(text[6], config.text.history.hints);
    assert_eq!(rows[4].1.style.bg, theme::shade().bg, "选中的整行铺底色");
    assert_eq!(rows[4].1.width(), 40, "时间贴着右边");
    let area = Rect::new(0, 10, 40, 7);
    assert_eq!(index_at(area, &rows, 14), Some(0));
    assert_eq!(index_at(area, &rows, 12), Some(2));
    assert_eq!(index_at(area, &rows, 10), None, "标题那一行点不中");
}

#[test]
fn commands_lines_pastes_and_hits_are_marked() {
    let _theme = theme::hold();
    let config = Config::builtin().unwrap();
    let now = Instant::now();
    let label = "[已粘贴 18 行]";
    let pasted = Sent {
        draft: Draft::from_pasted(
            &format!("{label} 看看报错"),
            &[(label.to_string(), "很长的报错".to_string())],
        ),
        at: now,
    };
    let list = [
        sent("/theme 换一套", 0, now),
        sent("第一行\n第二行\n第三行", 60, now),
        pasted,
    ];
    let found: Vec<&Sent> = list.iter().collect();
    let rows = lines(&History::default(), &found, 60, &config, now);
    let span = |row: usize, text: &str| {
        rows[row]
            .1
            .spans
            .iter()
            .find(|s| s.content.contains(text))
            .map(|s| s.style)
            .unwrap_or_else(|| panic!("第 {row} 行没有「{text}」：{:?}", rows[row].1))
    };
    // 最新的在最底下：第 4 行是命令，第 3 行是好几行的，第 2 行是带粘贴块的。
    assert_eq!(span(4, "/theme").fg, theme::accent().fg, "命令名强调色");
    let multi = rows[3].1.to_string();
    assert!(multi.contains("第一行") && multi.contains(" · +2 行") && !multi.contains("第二行"));
    assert_eq!(span(2, label), theme::chip(), "粘贴块照输入框里的样子");
    // 搜的字标出来。
    let history = History {
        query: "read".into(),
        ..History::default()
    };
    let list = [sent("看看 README 再说", 0, now)];
    let found: Vec<&Sent> = list.iter().collect();
    let rows = lines(&history, &found, 60, &config, now);
    assert!(plain(&rows)[0].starts_with("── 历史  1 条 · 搜索：read ─"));
    let hit = rows[2]
        .1
        .spans
        .iter()
        .find(|s| s.content == "READ")
        .map(|s| s.style)
        .unwrap();
    assert!(hit.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(hit.fg, theme::accent().fg);
}

#[test]
fn a_long_entry_is_clipped_and_keeps_its_time() {
    let config = Config::builtin().unwrap();
    let now = Instant::now();
    let long: String = "很长的一句话".repeat(10);
    let list = [sent(&long, 0, now)];
    let found: Vec<&Sent> = list.iter().collect();
    let rows = lines(&History::default(), &found, 30, &config, now);
    let row = plain(&rows)[2].clone();
    assert!(row.contains('…') && row.ends_with("刚才"), "{row}");
    assert_eq!(rows[2].1.width(), 30);
}

#[test]
fn nothing_found_says_so() {
    let config = Config::builtin().unwrap();
    let history = History {
        query: "xyz".into(),
        ..History::default()
    };
    let rows = lines(&history, &[], 40, &config, Instant::now());
    let text = plain(&rows);
    assert!(text[0].starts_with("── 历史  0 条 · 搜索：xyz ─"));
    assert_eq!(text[2], format!("  {}", config.text.history.empty));
}

#[test]
fn tab_shows_the_selected_one_in_full_and_caps_long_ones() {
    let config = Config::builtin().unwrap();
    let now = Instant::now();
    let mut history = History::default();
    history.toggle_full();
    let list = [sent("第一行\n第二行", 0, now), sent("早的", 0, now)];
    let found: Vec<&Sent> = list.iter().collect();
    let rows = lines(&history, &found, 40, &config, now);
    let text = plain(&rows);
    assert!(text[2].starts_with("  早的"));
    assert!(text[3].starts_with("❯ 第一行") && text[3].ends_with("刚才"));
    assert_eq!(text[4], "  第二行", "和一行时的字对齐");
    // 展开的两行都算这一条：点哪一行都是它。
    assert_eq!((rows[3].0, rows[4].0), (Some(0), Some(0)));
    // 太长的：最多 history_preview_rows 行，最后一行写还有几行。
    let long: String = (1..=20).map(|n| format!("第 {n} 行\n")).collect();
    let list = [sent(&long, 0, now)];
    let found: Vec<&Sent> = list.iter().collect();
    let rows = lines(&history, &found, 40, &config, now);
    let cap = config.layout.history_preview_rows;
    let text = plain(&rows);
    assert_eq!(rows.len(), 2 + cap + 2);
    assert_eq!(text[2 + cap - 1], format!("  ⋮ 还有 {} 行", 20 - (cap - 1)));
}
