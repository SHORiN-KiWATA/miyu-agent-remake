//! 输入历史列表（蓝图 `tui.md`「输入历史列表」）：照后台面板的样子（`panel.rs`），标题后面是条数和搜的字，
//! 下面对得上的几条，最新的贴着底，越早越往上；选中的停在正中间，到头才往边上走（照命令列表的 [`window`]）。
//! 每条右边写多久以前发的；命令名、好几行的、粘贴块、搜到的字各有记号。`Tab` 展开着时，选中的那一条写全文。
//!
//! 列表排成哪几行、每一行是哪一条，都由 [`lines`] 定：占几行、画什么、鼠标点的是哪一条，照同一份。

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::panel;
use crate::config::{Config, HistoryTexts};
use crate::history::History;
use crate::input::{Sent, pieces};
use crate::menu::window;
use crate::theme;

/// 排好的一行：是对得上的第几条（标题、空行、按键提示、「没有对得上的」是 `None`；展开的一条连「还有几行」都算它），
/// 和画出来的样子。
pub type Row = (Option<usize>, Line<'static>);

/// 列表从上往下的每一行。`matches` 是对得上的几条，最新的在前；`width` 是能写几列；`now` 算多久以前。
pub fn lines(
    history: &History,
    matches: &[&Sent],
    width: u16,
    config: &Config,
    now: Instant,
) -> Vec<Row> {
    let words = &config.text.history;
    let mut meta = vec![Span::styled(
        words.count.replace("{count}", &matches.len().to_string()),
        theme::dim(),
    )];
    if !history.query.is_empty() {
        meta.push(Span::styled(words.query.clone(), theme::dim()));
        meta.push(Span::raw(history.query.clone()));
    }
    let mut out: Vec<Row> = panel::head(&words.title, meta, width)
        .into_iter()
        .map(|line| (None, line))
        .collect();
    if matches.is_empty() {
        let empty = vec![Span::styled(words.empty.clone(), theme::dim())];
        out.push((None, panel::item(false, empty, None, width)));
    }
    let rows = config.layout.history_rows.max(1);
    let top = window(history.selected, matches.len(), rows);
    let end = (top + rows).min(matches.len());
    for i in (top..end).rev() {
        let picked = i == history.selected;
        let ago = Span::styled(ago(matches[i].at, now, words), theme::dim());
        if picked && history.expanded {
            out.extend(full(i, matches[i], ago, width, config));
            continue;
        }
        let content = content(matches[i], &history.query, picked, words);
        out.push((Some(i), panel::item(picked, content, Some(ago), width)));
    }
    out.extend(
        panel::hints(&words.hints)
            .into_iter()
            .map(|line| (None, line)),
    );
    out
}

/// 多久以前发的：一分钟以内「刚才」，再往后几分钟、几小时。
fn ago(at: Instant, now: Instant, words: &HistoryTexts) -> String {
    let secs = now.saturating_duration_since(at).as_secs();
    if secs < 60 {
        words.now.clone()
    } else if secs < 3600 {
        words.minutes.replace("{n}", &(secs / 60).to_string())
    } else {
        words.hours.replace("{n}", &(secs / 3600).to_string())
    }
}

/// 一行的字：只写第一行，后面还有的暗色跟「 · +N 行」；命令名强调色，粘贴块照输入框里的样子，搜到的字标出来。
fn content(sent: &Sent, query: &str, picked: bool, words: &HistoryTexts) -> Vec<Span<'static>> {
    let text = sent.draft.text.trim_end_matches('\n');
    let first = text.split('\n').next().unwrap_or_default();
    let extra = text.split('\n').count() - 1;
    let base = if picked {
        theme::picked()
    } else {
        Style::new()
    };
    let mut styles = vec![base; first.len()];
    if first.starts_with('/') {
        let end = first.find(char::is_whitespace).unwrap_or(first.len());
        paint(&mut styles, 0, end, |s| s.patch(theme::accent()));
    }
    for block in sent.draft.blocks.iter().filter(|b| b.end <= first.len()) {
        paint(&mut styles, block.start, block.end, |_| theme::chip());
    }
    let hit = theme::accent().add_modifier(Modifier::UNDERLINED);
    for (start, end) in hits(first, query) {
        paint(&mut styles, start, end, |s| s.patch(hit));
    }
    let mut spans = group(first, &styles);
    if extra > 0 {
        let more = words.lines.replace("{count}", &extra.to_string());
        spans.push(Span::styled(more, theme::dim()));
    }
    spans
}

/// `[start, end)` 这几个字节的样子照 `f` 改。
fn paint(styles: &mut [Style], start: usize, end: usize, f: impl Fn(Style) -> Style) {
    let end = end.min(styles.len());
    for style in &mut styles[start..end] {
        *style = f(*style);
    }
}

/// 连着一样样子的字并成一段。
fn group(text: &str, styles: &[Style]) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut from = 0;
    for (at, _) in text.char_indices().skip(1) {
        if styles[at] != styles[from] {
            out.push(Span::styled(text[from..at].to_string(), styles[from]));
            from = at;
        }
    }
    if from < text.len() {
        out.push(Span::styled(text[from..].to_string(), styles[from]));
    }
    out
}

/// `query` 在 `text` 里出现的地方（字节范围），不分大小写，一个字一个字比。
fn hits(text: &str, query: &str) -> Vec<(usize, usize)> {
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let wanted: Vec<char> = query.chars().map(lower).collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + wanted.len() <= chars.len() {
        let same = chars[i..i + wanted.len()]
            .iter()
            .zip(&wanted)
            .all(|((_, c), w)| lower(*c) == *w);
        if same {
            let end = chars
                .get(i + wanted.len())
                .map_or(text.len(), |(at, _)| *at);
            out.push((chars[i].0, end));
            i += wanted.len();
        } else {
            i += 1;
        }
    }
    out
}

/// 展开的一条：原来的换行照留，太长的折行，和一行时的字对齐；最多 `history_preview_rows` 行，再长的最后一行写还有几行。
fn full(index: usize, sent: &Sent, ago: Span<'static>, width: u16, config: &Config) -> Vec<Row> {
    let wrapped = pieces(sent.draft.text.trim_end(), width.saturating_sub(2).max(1));
    let cap = config.layout.history_preview_rows.max(2);
    let shown = if wrapped.len() > cap {
        cap - 1
    } else {
        wrapped.len()
    };
    let mut out: Vec<Row> = Vec::new();
    for (n, (piece, _)) in wrapped.iter().take(shown).enumerate() {
        let content = vec![Span::styled(piece.clone(), theme::picked())];
        let line = if n == 0 {
            panel::item(true, content, Some(ago.clone()), width)
        } else {
            panel::more(true, content, width)
        };
        out.push((Some(index), line));
    }
    let more = wrapped.len() - shown;
    if more > 0 {
        let note = config
            .text
            .history
            .more
            .replace("{count}", &more.to_string());
        let line = panel::more(true, vec![Span::styled(note, theme::dim())], width);
        out.push((Some(index), line));
    }
    out
}

/// 画列表。
pub fn draw(frame: &mut Frame, area: Rect, rows: Vec<Row>) {
    let lines: Vec<Line> = rows.into_iter().map(|(_, line)| line).collect();
    frame.render_widget(Paragraph::new(lines), area);
}

/// 屏幕上第 `y` 行是对得上的第几条；「历史：」那一行、空着的地方是 `None`。
pub fn index_at(area: Rect, rows: &[Row], y: u16) -> Option<usize> {
    rows.get(usize::from(y.checked_sub(area.y)?))?.0
}

#[cfg(test)]
mod tests;
