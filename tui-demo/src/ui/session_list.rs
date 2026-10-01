//! 会话列表的框（蓝图 `tui.md`「会话列表 `/sessions`」第 1、2 条）：和输入历史列表一个位置、一个样子。上边框写「会话」、
//! 几个、打的字；一行一个会话：置顶的打头一个记号，标题（没起名的暗色），暗色短编号，有工作目录的接着写；右边写
//! 「当前」「在忙」或者多久以前有过动静。勾上的行首一个紫色的勾（主题的 `picked`，2026-10-01 项目主人要紫色）。

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde::Deserialize;

use super::panel::{self, Chrome, Row};
use crate::config::Config;
use crate::core::SessionInfo;
use crate::session_list::{SessionList, short};
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
    /// 有一轮在跑的右边写的。
    pub busy: String,
    /// 多久以前：一分钟以内、几分钟、几小时、几天。
    pub now: String,
    /// `{n}` 分钟前。
    pub minutes: String,
    /// `{n}` 小时前。
    pub hours: String,
    /// `{n}` 天前。
    pub days: String,
    /// 勾了几个，`{count}`。
    pub ticked: String,
    /// 按了一次 `Ctrl+D`，`{title}`。
    pub delete_again: String,
    /// 按了一次 `Ctrl+D`、要删好几个，`{count}`。
    pub delete_many: String,
    /// 删了好几个，`{count}`。
    pub deleted_many: String,
    /// 删了，`{title}`。
    pub deleted: String,
    /// 正在用的不能删。
    pub delete_current: String,
    /// 置顶了、取消了。
    pub pinned: String,
    /// 取消置顶了。
    pub unpinned: String,
}

/// 框（标题、提示）、排好的行、每一行是对得上的第几个；`current` 是正在用的会话，放不下 `max` 行时离选中的远的少露。
pub fn lines(
    list: &SessionList,
    current: Option<&str>,
    config: &Config,
    width: u16,
    max: usize,
) -> (Chrome, Vec<Line<'static>>, Vec<Option<usize>>) {
    let texts = &config.text.sessions;
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
    if !list.ticked.is_empty() {
        let count = list.ticked.len().to_string();
        meta.push(Span::styled(
            texts.ticked.replace("{count}", &count),
            theme::dim(),
        ));
    }
    let chrome = Chrome::new(&texts.title, meta).hint(&texts.hint);
    let note = |text: &str| {
        let line = panel::item(
            false,
            vec![Span::styled(text.to_string(), theme::faint())],
            None,
            width,
        );
        (Vec::from([line]), vec![None])
    };
    if !list.loaded || found.is_empty() {
        let (lines, map) = note(if list.loaded {
            &texts.empty
        } else {
            &texts.loading
        });
        return (chrome, lines, map);
    }
    let now = jiff::Timestamp::now();
    // 最多露 `session_rows` 行，选中的停在正中间（第 1 条）。
    let shown = config.layout.session_rows.max(1);
    let top = crate::menu::top(list.selected, list.pinned, found.len(), shown);
    let ticking = !list.ticked.is_empty();
    let rows: Vec<Row> = found
        .iter()
        .enumerate()
        .skip(top)
        .take(shown)
        .map(|(i, info)| {
            let here = current == Some(info.session.as_str());
            let right = if here {
                Some(texts.current.clone())
            } else if info.busy {
                Some(texts.busy.clone())
            } else {
                info.last_active.map(|at| ago(at, now, texts))
            };
            let right = right.map(|r| Span::styled(r, theme::faint()));
            let mut content = content(info, config);
            if ticking {
                let mark = &config.layout.tick_mark;
                let mark = if list.ticked.contains(&info.session) {
                    mark.clone()
                } else {
                    " ".repeat(mark.chars().count())
                };
                content.insert(0, Span::styled(mark, theme::picked()));
            }
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

/// 一行的字：置顶记号、标题、短编号、工作目录。
fn content(info: &SessionInfo, config: &Config) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    if info.pinned {
        spans.push(Span::styled(config.icons.pin.clone(), theme::accent()));
    }
    spans.push(match &info.title {
        Some(title) => Span::raw(title.clone()),
        None => Span::styled(config.text.untitled.clone(), theme::dim()),
    });
    spans.push(Span::styled(
        format!("  #{}", short(&info.session)),
        theme::dim(),
    ));
    if let Some(cwd) = &info.cwd {
        let cwd = short_path(cwd, config.layout.session_cwd_width);
        spans.push(Span::styled(format!("  {cwd}"), theme::faint()));
    }
    spans
}

/// 工作目录写短：家目录写成 `~`，长过 `width` 列的只写最后两层、前面 `…/`（第 1 条）。
fn short_path(cwd: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    let home = std::env::var("HOME").unwrap_or_default();
    let cwd = match cwd.strip_prefix(home.as_str()) {
        Some(rest) if !home.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
            format!("~{rest}")
        }
        _ => cwd.to_string(),
    };
    if cwd.width() <= width {
        return cwd;
    }
    let parts: Vec<&str> = cwd.trim_end_matches('/').rsplit('/').take(2).collect();
    let tail: Vec<&str> = parts.into_iter().rev().collect();
    format!("…/{}", tail.join("/"))
}

/// 多久以前有过动静。
fn ago(at: jiff::Timestamp, now: jiff::Timestamp, texts: &Texts) -> String {
    let secs = now.duration_since(at).as_secs().max(0);
    let n = |d: i64, words: &str| words.replace("{n}", &(secs / d).to_string());
    match secs {
        0..60 => texts.now.clone(),
        60..3600 => n(60, &texts.minutes),
        3600..86400 => n(3600, &texts.hours),
        _ => n(86400, &texts.days),
    }
}

#[cfg(test)]
mod tests {
    use super::short_path;

    #[test]
    fn a_long_directory_keeps_its_last_two_levels() {
        assert_eq!(short_path("~/src/miyu", 24), "~/src/miyu");
        assert_eq!(
            short_path("/tmp/claude-1000/-home-shorin/scratchpad/work", 24),
            "…/scratchpad/work"
        );
        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() {
            assert_eq!(short_path(&format!("{home}/src"), 24), "~/src");
            assert_eq!(
                short_path(&format!("{home}x/src"), 99),
                format!("{home}x/src"),
                "只认整层"
            );
        }
    }
}
