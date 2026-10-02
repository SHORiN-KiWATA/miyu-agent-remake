//! `/effort` 的框（蓝图 `tui.md`「配置与模型」思考强度，2026-10-02 项目主人定：只要这个框）：和 `/language` 一个位置、
//! 一个样子。第一行「默认（供应商定）」，下面照这个模型的几级一级一行；配置的默认那一级前面一个强调色的 `●`。

use ratatui::text::{Line, Span};
use serde::Deserialize;

use super::panel::{self, Chrome, Row};
use crate::config::Config;
use crate::core::Efforts;
use crate::theme;

/// 框里的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 上边框写的，`{model}` 是模型的显示名。
    pub title: String,
    /// 核心还没交回来时的标题。
    pub loading_title: String,
    /// 下边框的按键提示。
    pub hint: String,
    /// 第一行：去掉这一项，供应商那头定。
    pub default: String,
    /// 核心还没交回来。
    pub loading: String,
    /// 这个模型一级都没有。
    pub none: String,
    /// 用的是模型池，不让改（2026-10-02 项目主人定）。
    pub need_model: String,
}

/// 框、排好的行、每一行是第几行（第 0 行默认）。`target` 是 `None` 的还在等核心。
pub fn lines(
    target: Option<&Efforts>,
    selected: usize,
    config: &Config,
    width: u16,
    max: usize,
) -> (Chrome, Vec<Line<'static>>, Vec<Option<usize>>) {
    let texts = &config.text.effort;
    let note = |title: &str, text: &str| {
        let chrome = Chrome::new(title, Vec::new()).hint(&texts.hint);
        let line = panel::item(
            false,
            vec![Span::styled(text.to_string(), theme::faint())],
            None,
            width,
        );
        (chrome, vec![line], vec![None])
    };
    let Some(target) = target else {
        return note(&texts.loading_title, &texts.loading);
    };
    let title = texts.title.replace("{model}", &target.name);
    if target.levels.is_empty() {
        return note(&title, &texts.none);
    }
    let chrome = Chrome::new(&title, Vec::new()).hint(&texts.hint);
    // 配置的默认那一级前面一个强调色的 `●`，别的空两格（照会话列表，2026-10-02 项目主人）。
    let mark = |on: bool| {
        if on {
            Span::styled(format!("{} ", config.layout.current_mark), theme::accent())
        } else {
            Span::raw("  ")
        }
    };
    let rows: Vec<Row> = std::iter::once((texts.default.clone(), target.current.is_none()))
        .chain(
            target
                .levels
                .iter()
                .map(|l| (l.clone(), target.current.as_deref() == Some(l.as_str()))),
        )
        .enumerate()
        .map(|(i, (name, on))| {
            (
                Some(i),
                panel::item(i == selected, vec![mark(on), Span::raw(name)], None, width),
            )
        })
        .collect();
    let rows = panel::fit(rows, Some(selected), max);
    let (map, lines) = rows.into_iter().unzip();
    (chrome, lines, map)
}

/// 一开框选着哪一行：配置的默认那一级；没配的是第 0 行（默认）。
pub fn initial(target: &Efforts) -> usize {
    target
        .current
        .as_deref()
        .and_then(|c| target.levels.iter().position(|l| l == c))
        .map_or(0, |i| i + 1)
}
