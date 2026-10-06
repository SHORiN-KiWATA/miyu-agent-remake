//! 输入历史列表排成的行（蓝图 `tui.md`「输入历史列表」）：和命令列表、后台面板一个框，每条右边写多久以前，
//! 命令名、好几行的、粘贴块、搜到的字各有记号。

use std::time::{Duration, SystemTime};

use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::{index_at, lines};
use crate::config::Config;
use crate::history::History;
use crate::input::{Draft, Sent};
use crate::theme;

fn sent(text: &str, ago: u64, now: SystemTime) -> Sent {
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

/// 上边框写的字。
fn title(chrome: &crate::ui::panel::Chrome) -> String {
    chrome.title.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn it_looks_like_the_background_panel_and_clicks_find_their_entry() {
    let _theme = theme::hold();
    let config = Config::builtin().unwrap();
    let now = SystemTime::now();
    let list = [
        sent("新的", 10, now),
        sent("中间", 300, now),
        sent("早的", 7200, now),
    ];
    let found: Vec<&Sent> = list.iter().collect();
    let (chrome, rows) = lines(&History::default(), &found, 40, &config, now, usize::MAX);
    let text = plain(&rows);
    // 2026-09-30 项目主人：三样一个框，标题嵌在上边框，按键提示嵌在下边框，框里不空行。
    assert_eq!(title(&chrome), "历史  3 条");
    assert_eq!(
        chrome.hint.as_deref(),
        Some(config.text.history.hints.as_str())
    );
    assert!(
        text[0].starts_with("  早的") && text[0].ends_with("2 小时前"),
        "{text:?}"
    );
    assert!(text[1].starts_with("  中间") && text[1].ends_with("5 分钟前"));
    assert!(text[2].starts_with("❯ 新的") && text[2].ends_with("刚才"));
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[2].1.style,
        theme::picked_bar(),
        "选中的铺强调色的底、字深色"
    );
    assert_eq!(rows[2].1.width(), 40, "时间贴着右边");
    // 框里放字的那一块从第 10 行起。
    let text_area = Rect::new(0, 10, 40, 3);
    assert_eq!(index_at(text_area, &rows, 12), Some(0));
    assert_eq!(index_at(text_area, &rows, 10), Some(2));
    assert_eq!(index_at(text_area, &rows, 9), None, "框的上边点不中");
}

#[test]
fn commands_lines_pastes_and_hits_are_marked() {
    let _theme = theme::hold();
    let config = Config::builtin().unwrap();
    let now = SystemTime::now();
    let label = "[已粘贴 18 行]";
    let pasted = Sent {
        draft: Draft::from_pasted(
            &format!("{label} 看看报错"),
            &[(label.to_string(), "很长的报错".to_string())],
        ),
        at: now,
    };
    // 选中的是最新的那一条（「别的话」）：记号看的是没选中的几条。
    let list = [
        sent("别的话", 0, now),
        sent("/theme 换一套", 0, now),
        sent("第一行\n第二行\n第三行", 60, now),
        pasted,
    ];
    let found: Vec<&Sent> = list.iter().collect();
    let (_, rows) = lines(&History::default(), &found, 60, &config, now, usize::MAX);
    let span = |row: usize, text: &str| {
        rows[row]
            .1
            .spans
            .iter()
            .find(|s| s.content.contains(text))
            .map(|s| s.style)
            .unwrap_or_else(|| panic!("第 {row} 行没有「{text}」：{:?}", rows[row].1))
    };
    // 最新的在最底下：第 2 行是命令，第 1 行是好几行的，第 0 行是带粘贴块的。
    assert_eq!(span(2, "/theme").fg, theme::accent().fg, "命令名强调色");
    assert_eq!(span(2, "换一套").fg, theme::dim().fg, "没选中的字暗");
    let multi = rows[1].1.to_string();
    assert!(multi.contains("第一行") && multi.contains(" · +2 行") && !multi.contains("第二行"));
    assert_eq!(span(0, label), theme::chip(), "粘贴块照输入框里的样子");
    // 搜的字标出来。
    let history = History {
        query: "read".into(),
        ..History::default()
    };
    let list = [sent("别的", 0, now), sent("看看 README 再说", 0, now)];
    let found: Vec<&Sent> = list.iter().collect();
    let (chrome, rows) = lines(&history, &found, 60, &config, now, usize::MAX);
    assert_eq!(title(&chrome), "历史  2 条 · 搜索：read");
    let hit = rows[0]
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
    let now = SystemTime::now();
    let long: String = "很长的一句话".repeat(10);
    let list = [sent(&long, 0, now)];
    let found: Vec<&Sent> = list.iter().collect();
    let (_, rows) = lines(&History::default(), &found, 30, &config, now, usize::MAX);
    let row = plain(&rows)[0].clone();
    assert!(row.contains('…') && row.ends_with("刚才"), "{row}");
    assert_eq!(rows[0].1.width(), 30);
}

#[test]
fn nothing_found_says_so() {
    let config = Config::builtin().unwrap();
    let history = History {
        query: "xyz".into(),
        ..History::default()
    };
    let (chrome, rows) = lines(&history, &[], 40, &config, SystemTime::now(), usize::MAX);
    assert_eq!(title(&chrome), "历史  0 条 · 搜索：xyz");
    assert_eq!(plain(&rows), [format!("  {}", config.text.history.empty)]);
}

#[test]
fn tab_shows_the_selected_one_in_full_and_caps_long_ones() {
    let config = Config::builtin().unwrap();
    let now = SystemTime::now();
    let list = [sent("第一行\n第二行", 0, now), sent("早的", 0, now)];
    let mut history = History::default();
    history.toggle_full(list[0].at);
    let found: Vec<&Sent> = list.iter().collect();
    let (_, rows) = lines(&history, &found, 40, &config, now, usize::MAX);
    let text = plain(&rows);
    assert!(text[0].starts_with("  早的"));
    assert!(text[1].starts_with("❯ 第一行") && text[1].ends_with("刚才"));
    assert_eq!(text[2], "  第二行", "和一行时的字对齐");
    // 展开的两行都算这一条：点哪一行都是它。
    assert_eq!((rows[1].0, rows[2].0), (Some(0), Some(0)));
    // 太长的：最多 history_preview_rows 行，最后一行写还有几行。
    let long: String = (1..=20).map(|n| format!("第 {n} 行\n")).collect();
    let list = [sent(&long, 0, now)];
    let mut history = History::default();
    history.toggle_full(list[0].at);
    let found: Vec<&Sent> = list.iter().collect();
    let (_, rows) = lines(&history, &found, 40, &config, now, usize::MAX);
    let cap = config.layout.history_preview_rows;
    let text = plain(&rows);
    assert_eq!(rows.len(), cap);
    assert_eq!(text[cap - 1], format!("  ⋮ 还有 {} 行", 20 - (cap - 1)));
}

#[test]
fn an_expanded_entry_stays_open_and_others_do_not_follow() {
    // 2026-09-29 项目主人：展开的那一条光标移走不收起，移到别的条上也不跟着展开。
    let _theme = theme::hold();
    let config = Config::builtin().unwrap();
    let now = SystemTime::now();
    let list = [
        sent("第一行\n第二行", 0, now),
        sent("甲\n乙", 60, now),
        sent("丙\n丁", 120, now),
    ];
    let found: Vec<&Sent> = list.iter().collect();
    let mut history = History::default();
    history.toggle_full(list[0].at);
    history.older(3);
    let (_, rows) = lines(&history, &found, 40, &config, now, usize::MAX);
    let text = plain(&rows);
    assert!(
        text.contains(&"❯ 甲 · +1 行".to_string())
            || text.iter().any(|l| l.starts_with("❯ 甲 · +1 行")),
        "新选中的不跟着展开：{text:?}"
    );
    assert!(
        text.iter().any(|l| l.starts_with("  第一行")),
        "移走了照旧展开着"
    );
    assert!(text.iter().any(|l| l == "  第二行"));
    let first = rows
        .iter()
        .find(|(_, l)| l.to_string().starts_with("  第一行"))
        .unwrap();
    assert_ne!(first.1.style, theme::picked_bar(), "没选中的不铺底色");
    // 再按一下 Tab 收起它自己那一条；可以同时展开好几条。
    history.toggle_full(list[1].at);
    let text = plain(&lines(&history, &found, 40, &config, now, usize::MAX).1);
    assert!(text.iter().any(|l| l.starts_with("❯ 甲")) && text.iter().any(|l| l == "  乙"));
    assert!(text.iter().any(|l| l == "  第二行"), "两条同时展开");
    history.toggle_full(list[0].at);
    let text = plain(&lines(&history, &found, 40, &config, now, usize::MAX).1);
    assert!(!text.iter().any(|l| l == "  第二行"), "再按收回");
}
