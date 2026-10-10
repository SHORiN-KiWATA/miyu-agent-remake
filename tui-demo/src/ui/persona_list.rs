//! 新会话的人格框、预设框（蓝图 `tui.md`「新会话：人格、工作区」第 1、3 条）：和 `/language`、`/model` 一个位置、一个样子。一个
//! 人格一行，只写名字（没有的写编号）；默认的那个右边暗色写「默认」；写错的暗着、后面黄字写原因，选不了。

use ratatui::text::Span;
use serde::Deserialize;

use super::panel::{self, Chrome, Row};
use crate::app::new_session::Kind;
use crate::core::Persona;
use crate::theme;

/// 人格框和人格那一行的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 上边框写的。
    pub title: String,
    /// 下边框的按键提示。
    pub hint: String,
    /// 默认人格右边写的。
    pub default_mark: String,
    /// 不带人格那一行的名字（2026-10-08 项目主人定叫「无人格」）。
    pub none: String,
    /// 它后面暗色写的。
    pub none_note: String,
    /// 开了会话再打 `/persona`。
    pub locked: String,
    /// 首页框下面那一行、侧边栏：`{name}` 是选的人格。
    pub chosen: String,
    /// 预设框上边框写的。
    pub preset_title: String,
    /// 默认预设用不了时按 `Esc`（开会话一定要有预设，核心 Y12）。
    pub preset_need: String,
    /// 开了会话再打 `/preset`。
    pub preset_locked: String,
    /// 首页框下面那一行、侧边栏：`{name}` 是选的预设。
    pub preset_chosen: String,
}

/// 框（标题照是人格还是预设、提示）、排好的行、每一行是第几个。
pub fn lines(
    list: &[Persona],
    default: Option<&str>,
    kind: Kind,
    selected: usize,
    texts: &Texts,
    width: u16,
    max: usize,
) -> (
    Chrome,
    Vec<ratatui::text::Line<'static>>,
    Vec<Option<usize>>,
) {
    let title = match kind {
        Kind::Persona => &texts.title,
        Kind::Preset => &texts.preset_title,
    };
    let chrome = Chrome::new(title, Vec::new()).hint(&texts.hint);
    let rows: Vec<Row> = list
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (content, right) = match &p.problem {
                Some(problem) => (
                    vec![
                        Span::styled(p.label().to_string(), theme::dim()),
                        Span::styled(format!("  {problem}"), theme::warn()),
                    ],
                    None,
                ),
                None => {
                    let mark = (default == Some(p.id.as_str()))
                        .then(|| Span::styled(texts.default_mark.clone(), theme::dim()));
                    let mut content = vec![Span::raw(p.label().to_string())];
                    // 「无人格」那一行（编号空着）后面暗色写它意味着什么。
                    if p.id.is_empty() {
                        content.push(Span::styled(format!("  {}", texts.none_note), theme::dim()));
                    }
                    (content, mark)
                }
            };
            (Some(i), panel::item(i == selected, content, right, width))
        })
        .collect();
    let rows = panel::fit(rows, Some(selected), max);
    let (map, lines) = rows.into_iter().unzip();
    (chrome, lines, map)
}
