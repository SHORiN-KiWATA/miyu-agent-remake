//! 界面语言的框（蓝图 `tui.md`「界面语言」，2026-09-30 项目主人：做成列表让选）：和帮助框、后台面板一个位置、一个样子。
//! 一种一行，写它自己的名字；现在用的那种右边暗色写「当前」；选中的照别的列表铺底。

use ratatui::text::{Line, Span};
use serde::Deserialize;

use super::panel::{self, Chrome, Row};
use crate::config::Config;
use crate::theme;

/// 框里的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 上边框写的。
    pub title: String,
    /// 下边框的按键提示。
    pub hint: String,
    /// 现在用的那种右边写的。
    pub current: String,
}

/// 框（标题、提示）、排好的行、每一行是第几种；选中第 `selected` 种，放不下 `max` 行时从离它最远的起少露。
pub fn lines(
    config: &Config,
    selected: usize,
    width: u16,
    max: usize,
) -> (Chrome, Vec<Line<'static>>, Vec<Option<usize>>) {
    let texts = &config.text.languages;
    let chrome = Chrome::new(&texts.title, Vec::new()).hint(&texts.hint);
    let current = config.language.code();
    let rows: Vec<Row> = config
        .language_table
        .languages
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let right =
                (entry.code == current).then(|| Span::styled(texts.current.clone(), theme::dim()));
            let content = vec![Span::raw(entry.name.clone())];
            (Some(i), panel::item(i == selected, content, right, width))
        })
        .collect();
    let rows = panel::fit(rows, Some(selected), max);
    let (map, lines) = rows.into_iter().unzip();
    (chrome, lines, map)
}

#[cfg(test)]
mod tests {
    use super::lines;
    use crate::config::Config;

    #[test]
    fn each_language_is_one_row_by_its_own_name_and_the_current_one_says_so() {
        let config = Config::builtin().unwrap();
        let (chrome, rows, map) = lines(&config, 2, 30, 10);
        let text: Vec<String> = rows.iter().map(|l| l.to_string()).collect();
        assert_eq!(text.len(), 3);
        assert!(
            text[0].starts_with("  中文") && text[0].trim_end().ends_with("当前"),
            "{text:?}"
        );
        assert_eq!(text[1].trim_end(), "  English");
        assert_eq!(text[2].trim_end(), "❯ 日本語", "选中的写 ❯");
        assert_eq!(map, [Some(0), Some(1), Some(2)]);
        let title: String = chrome.title.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(title, "界面语言");
    }

    #[test]
    fn a_short_room_keeps_the_selected_row() {
        let config = Config::builtin().unwrap();
        let (_, rows, map) = lines(&config, 2, 30, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(map, [Some(2)]);
    }
}
