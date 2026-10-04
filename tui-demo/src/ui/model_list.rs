//! `/model` 的框（蓝图 `tui.md`「配置与模型」第 1 条）：和会话列表一个位置、一个样子。名字一列对齐，暗色写供应商和
//! 窗口（池写分法和成员数，挡位写指向哪个）；右边写「当前」、冷却还要几分钟（黄）、没有 key（整行暗）。

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde::Deserialize;
use unicode_width::UnicodeWidthStr;

use super::panel::{self, Chrome, Row};
use crate::config::Config;
use crate::core::{Choice, ChoiceState};
use crate::model_list::ModelList;
use crate::theme;

/// 框里的字。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// 上边框写的。
    pub title: String,
    /// 几个，`{count}`。
    pub count: String,
    /// 打了字时接在后面的。
    pub query: String,
    /// 下边框的按键提示。
    pub hint: String,
    /// 核心还没交回来。
    pub loading: String,
    /// 一个都对不上。
    pub empty: String,
    /// 正在用的这个右边写的。
    pub current: String,
    /// 在冷却，`{n}` 分钟后能用。
    pub cooling: String,
    /// 在冷却，不知道几时能用。
    pub cooling_bare: String,
    /// 没有 key。
    pub no_key: String,
    /// 选了在冷却的，`{n}`。
    pub refuse_cooling: String,
    /// 选了没有 key 的。
    pub refuse_no_key: String,
}

/// 框（标题、提示）、排好的行、每一行是对得上的第几个；`current` 是会话现在用的引用。
pub fn lines(
    list: &ModelList,
    current: Option<&str>,
    config: &Config,
    width: u16,
    max: usize,
) -> (Chrome, Vec<Line<'static>>, Vec<Option<usize>>) {
    let texts = &config.text.model_panel;
    let found = list.matches();
    let mut meta = vec![Span::styled(
        texts.count.replace("{count}", &found.len().to_string()),
        theme::dim(),
    )];
    if !list.query.is_empty() {
        meta.push(Span::styled(texts.query.clone(), theme::dim()));
        meta.push(Span::styled(
            list.query.clone(),
            Style::new().fg(Color::Reset),
        ));
    }
    let chrome = Chrome::new(&texts.title, meta).hint(&texts.hint);
    if !list.loaded || found.is_empty() {
        let note = if list.loaded {
            &texts.empty
        } else {
            &texts.loading
        };
        let line = panel::item(
            false,
            vec![Span::styled(note.clone(), theme::faint())],
            None,
            width,
        );
        return (chrome, vec![line], vec![None]);
    }
    let now = jiff::Timestamp::now();
    let shown = config.layout.session_rows.max(1);
    let top = crate::menu::top(list.selected, list.pinned, found.len(), shown);
    let column = found
        .iter()
        .map(|c| c.name.width())
        .max()
        .unwrap_or(0)
        .min(config.layout.session_title_width);
    let rows: Vec<Row> = found
        .iter()
        .enumerate()
        .skip(top)
        .take(shown)
        .map(|(i, choice)| {
            let off = choice.state == ChoiceState::NoKey;
            let name_style = if off { theme::dim() } else { Style::new() };
            let name = super::rows::clip(&choice.name, u16::try_from(column).unwrap_or(u16::MAX));
            let pad = column.saturating_sub(name.width());
            let content = vec![
                Span::styled(format!("{name}{}", " ".repeat(pad)), name_style),
                // 引用和窗口用 `dim`：原来 `faint` 太暗（2026-10-01 项目主人）；没有 key 的整行压暗。
                Span::styled(
                    format!("  {}", choice.detail),
                    if off { theme::faint() } else { theme::dim() },
                ),
            ];
            let right = right(choice, current, now, texts);
            (
                Some(i),
                panel::item(i == list.selected, content, right, width),
            )
        })
        .collect();
    let rows = panel::fit(rows, Some(list.selected), max);
    let (map, lines) = rows.into_iter().unzip();
    (chrome, lines, map)
}

/// 右边那一格：正在用的写「当前」，冷却中的写还要几分钟（黄），没有 key 的暗着写。
fn right(
    choice: &Choice,
    current: Option<&str>,
    now: jiff::Timestamp,
    texts: &Texts,
) -> Option<Span<'static>> {
    match choice.state {
        ChoiceState::Cooling(until) => {
            Some(Span::styled(cooling(until, now, texts), theme::warn()))
        }
        ChoiceState::NoKey => Some(Span::styled(texts.no_key.clone(), theme::faint())),
        ChoiceState::Ok if current == Some(choice.reference.as_str()) => {
            Some(Span::styled(texts.current.clone(), theme::faint()))
        }
        ChoiceState::Ok => None,
    }
}

/// 冷却还要几分钟（向上取整）；不知道几时、已经到点的只写在冷却。
pub fn cooling(until: Option<jiff::Timestamp>, now: jiff::Timestamp, texts: &Texts) -> String {
    let left = until.map_or(0, |until| until.duration_since(now).as_secs());
    if left <= 0 {
        return texts.cooling_bare.clone();
    }
    texts
        .cooling
        .replace("{n}", &((left + 59) / 60).to_string())
}

#[cfg(test)]
mod tests {
    use super::lines;
    use crate::config::Config;
    use crate::core::{Choice, ChoiceState};
    use crate::model_list::ModelList;

    fn choice(reference: &str, name: &str, state: ChoiceState) -> Choice {
        Choice {
            reference: reference.into(),
            name: name.into(),
            detail: "dev · 300k".into(),
            state,
        }
    }

    #[test]
    fn rows_say_current_cooling_and_no_key_on_the_right() {
        let config = Config::builtin().unwrap();
        let mut list = ModelList::default();
        let soon = jiff::Timestamp::now()
            .checked_add(jiff::SignedDuration::from_secs(90))
            .unwrap();
        list.replace(
            vec![
                choice("dev/m1", "Flash", ChoiceState::Ok),
                choice("dev/m2", "Pro", ChoiceState::Cooling(Some(soon))),
                choice("dev/m3", "Old", ChoiceState::NoKey),
            ],
            Some("dev/m1"),
        );
        let (_, rows, _) = lines(&list, Some("dev/m1"), &config, 80, 20);
        let text: Vec<String> = rows.iter().map(ToString::to_string).collect();
        assert!(
            text[0].contains("Flash") && text[0].trim_end().ends_with("当前"),
            "{text:#?}"
        );
        assert!(text[1].trim_end().ends_with("冷却 2m"), "{text:#?}");
        assert!(text[2].trim_end().ends_with("没有 key"), "{text:#?}");
        assert!(text.iter().all(|l| l.contains("dev · 300k")));
    }
}
