//! 界面语言的框（蓝图 `tui.md`「界面语言」，2026-09-30 项目主人：做成列表让选）：和帮助框、后台面板一个位置、一个样子。
//! 第一行是自动（跟随系统，括号里写系统认出来的那种；2026-10-01 项目主人加），下面一种一行，写它自己的名字；现在用的那一档
//! 右边暗色写「当前」；选中的照别的列表铺底。

use ratatui::text::{Line, Span};
use serde::Deserialize;

use super::panel::{self, Chrome, Row};
use crate::config::Config;
use crate::language::Language;
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
    /// 第一行：自动，`{name}` 是系统认出来的那种。
    pub auto: String,
    /// 换回自动以后提示的一句，`{name}` 同上。
    pub auto_switched: String,
}

/// 框（标题、提示）、排好的行、每一行是第几行（第 0 行自动）；选中第 `selected` 行，放不下 `max` 行时从离它最远的起少露。
/// `system` 是启动时照系统认出来的那种，写在自动那一行的括号里。
pub fn lines(
    config: &Config,
    system: &Language,
    selected: usize,
    width: u16,
    max: usize,
) -> (Chrome, Vec<Line<'static>>, Vec<Option<usize>>) {
    let texts = &config.text.languages;
    let chrome = Chrome::new(&texts.title, Vec::new()).hint(&texts.hint);
    let table = &config.language_table;
    let current = |on: bool| on.then(|| Span::styled(texts.current.clone(), theme::dim()));
    let auto = texts.auto.replace("{name}", table.name(system));
    let manual = table.languages.iter().map(|entry| {
        let on = !config.auto && entry.code == config.language.code();
        (entry.name.clone(), on)
    });
    let rows: Vec<Row> = std::iter::once((auto, config.auto))
        .chain(manual)
        .enumerate()
        .map(|(i, (name, on))| {
            let content = vec![Span::raw(name)];
            (
                Some(i),
                panel::item(i == selected, content, current(on), width),
            )
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
    use crate::language::LanguageTable;

    fn text(config: &Config, selected: usize, max: usize) -> (Vec<String>, Vec<Option<usize>>) {
        let zh = LanguageTable::builtin().unwrap().find("zh").unwrap();
        let (_, rows, map) = lines(config, &zh, selected, 40, max);
        (
            rows.iter()
                .map(|l| l.to_string().trim_end().to_string())
                .collect(),
            map,
        )
    }

    #[test]
    fn auto_comes_first_and_says_which_language_the_system_picked() {
        // 2026-10-01 项目主人：原来没有自动这一档。启动时是自动，「当前」标在第一行。
        let config = Config::builtin().unwrap();
        let (rows, map) = text(&config, 3, 10);
        assert_eq!(rows.len(), 4);
        assert!(
            rows[0].starts_with("  自动（跟随系统：中文）") && rows[0].ends_with("当前"),
            "{rows:?}"
        );
        assert_eq!(rows[1], "  中文", "自动时手动那几行都不标当前");
        assert_eq!(rows[2], "  English");
        assert_eq!(rows[3], "❯ 日本語", "选中的写 ❯");
        assert_eq!(map, [Some(0), Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn a_manual_choice_is_marked_on_its_own_row() {
        let table = LanguageTable::builtin().unwrap();
        let config = Config::load(&table.find("zh").unwrap(), false).unwrap();
        let (rows, _) = text(&config, 0, 10);
        assert_eq!(rows[0], "❯ 自动（跟随系统：中文）");
        assert!(
            rows[1].starts_with("  中文") && rows[1].ends_with("当前"),
            "{rows:?}"
        );
    }

    #[test]
    fn a_short_room_keeps_the_selected_row() {
        let config = Config::builtin().unwrap();
        let (rows, map) = text(&config, 3, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(map, [Some(3)]);
    }
}
