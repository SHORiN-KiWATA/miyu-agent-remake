//! 输入历史列表（蓝图 `tui.md`「输入历史列表」）：和命令列表、后台面板一个框（`panel.rs`），上边框写标题、条数和搜的字，
//! 下边框写按键提示；
//! 下面对得上的几条，最新的贴着底，越早越往上；选中的停在正中间，到头才往边上走（照命令列表的 [`window`]）。
//! 每条右边写多久以前发的；命令名、好几行的、粘贴块、搜到的字各有记号。`Tab` 展开着时，选中的那一条写全文。
//!
//! 列表排成哪几行、每一行是哪一条，都由 [`lines`] 定：占几行、画什么、鼠标点的是哪一条，照同一份。

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::panel::{self, Chrome};
use crate::config::{Config, HistoryTexts};
use crate::history::History;
use crate::input::{Sent, pieces};
use crate::menu::window;
use crate::theme;

/// 排好的一行：是对得上的第几条（「没有对得上的」是 `None`；展开的一条连「还有几行」都算它），和画出来的样子。
pub type Row = panel::Row;

/// 列表的框和框里从上往下的每一行。`matches` 是对得上的几条，最新的在前；`width` 是能写几列；`now` 算多久以前；
/// 框里最多 `max` 行（输入框上面剩下的，「窗口小的时候」第 1 条）。
pub fn lines(
    history: &History,
    matches: &[&Sent],
    width: u16,
    config: &Config,
    now: Instant,
    max: usize,
) -> (Chrome, Vec<Row>) {
    let words = &config.text.history;
    // 边框上的字和框线一个颜色，只有打的字原色（写明：不写的话照框线的颜色）。
    let mut meta = vec![Span::styled(
        words.count.replace("{count}", &matches.len().to_string()),
        theme::dim(),
    )];
    if !history.query.is_empty() {
        meta.push(Span::styled(words.query.clone(), theme::dim()));
        let typed = Style::new().fg(ratatui::style::Color::Reset);
        meta.push(Span::styled(history.query.clone(), typed));
    }
    let chrome = Chrome::new(&words.title, meta).hint(&words.hints);
    let mut out: Vec<Row> = Vec::new();
    if matches.is_empty() {
        let empty = vec![Span::styled(words.empty.clone(), theme::faint())];
        out.push((None, panel::item(false, empty, None, width)));
    }
    let rows = config.layout.history_rows.max(1);
    let top = window(history.selected, matches.len(), rows);
    let end = (top + rows).min(matches.len());
    for i in (top..end).rev() {
        let picked = i == history.selected;
        let ago = Span::styled(ago(matches[i].at, now, words), theme::faint());
        if history.is_expanded(matches[i].at) {
            out.extend(full(i, matches[i], picked, ago, width, config));
            continue;
        }
        let content = content(matches[i], &history.query, picked, words);
        out.push((Some(i), panel::item(picked, content, Some(ago), width)));
    }
    (chrome, panel::fit(out, Some(history.selected), max))
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
    // 没选中的字暗、命令名强调色；选中的颜色由选中的那一条的底色定（`panel::item`）。
    let base = if picked {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        theme::dim()
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

/// 展开的一条：原来的换行照留，太长的折行，和一行时的字对齐；选中着的品红、铺底色。最多 `history_preview_rows` 行，
/// 再长的最后一行写还有几行。
fn full(
    index: usize,
    sent: &Sent,
    picked: bool,
    ago: Span<'static>,
    width: u16,
    config: &Config,
) -> Vec<Row> {
    let style = if picked {
        Style::new().add_modifier(Modifier::BOLD)
    } else {
        theme::dim()
    };
    let wrapped = pieces(sent.draft.text.trim_end(), width.saturating_sub(2).max(1));
    let cap = config.layout.history_preview_rows.max(2);
    let shown = if wrapped.len() > cap {
        cap - 1
    } else {
        wrapped.len()
    };
    let mut out: Vec<Row> = Vec::new();
    for (n, (piece, _)) in wrapped.iter().take(shown).enumerate() {
        let content = vec![Span::styled(piece.clone(), style)];
        let line = if n == 0 {
            panel::item(picked, content, Some(ago.clone()), width)
        } else {
            panel::more(picked, content, width)
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
        let line = panel::more(picked, vec![Span::styled(note, theme::faint())], width);
        out.push((Some(index), line));
    }
    out
}

/// 画列表：`outer` 连框，`text` 是框里放字的那一块。
pub fn draw(frame: &mut Frame, outer: Rect, text: Rect, chrome: Chrome, rows: Vec<Row>) {
    let lines: Vec<Line> = rows.into_iter().map(|(_, line)| line).collect();
    panel::draw(frame, outer, text, chrome, lines);
}

/// 屏幕上第 `y` 行是对得上的第几条；`text` 是框里放字的那一块，框的边、空着的地方是 `None`。
pub fn index_at(text: Rect, rows: &[Row], y: u16) -> Option<usize> {
    rows.get(usize::from(y.checked_sub(text.y)?))?.0
}

#[cfg(test)]
mod tests;
