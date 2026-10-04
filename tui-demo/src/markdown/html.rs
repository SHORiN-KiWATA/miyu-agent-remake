//! 回答里的 HTML（蓝图 `tui.md`「她的回答：Markdown」第 12、15 条，「图片、公式和 mermaid 图」第 8、9 条）：
//! `<br>` 换行；`<img>` 当图片；`<svg>` 从头到 `</svg>` 收成一张图；`<details>` 画成能点开收起的一块；
//! 别的标签原样写出来。
//!
//! pulldown-cmark 把独占一块的 HTML 一行一条交过来，一行里的一个标签一条；`<details>` 和 `</details>` 中间的
//! Markdown 照常解析。所以这里只认标签，一块 `<details>` 的开关、标题、里面的内容从哪一行起，记在 [`Details`] 里。

use pulldown_cmark::Event;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::inline::{Piece, fold};
use super::{Container, FigureKind, MdLine, Renderer};
use crate::theme;
use scan::{Token, find, tokens};

/// 套着的一块 `<details>`。
pub(super) struct Details {
    /// 这一条回答里第几个（从 0 数）。
    index: usize,
    /// 展开着：写的 `open` 和点过没有，两样反一反。
    open: bool,
    /// 在收 `<summary>` 里的字；还没见到 `<summary>` 的是 `None`。
    summary: Option<String>,
    /// 标题那一行画了：里面的内容从 `out` 的第几行起；收着的，收尾时从这里截掉。
    body: Option<usize>,
}

impl Renderer<'_> {
    /// 一段 HTML：`block` 是独占一块的那种（一行一条，末尾带换行）。
    pub(super) fn html(&mut self, html: &str, block: bool) {
        let html = if block {
            html.trim_end_matches('\n')
        } else {
            html
        };
        // 原样写出来的（认不得的标签、标签外的字）：独占一块的，一行写完换行。
        let mut raw = false;
        for token in tokens(html) {
            match token {
                Token::Tag { name, closing, .. } if name == "br" && !closing => {
                    self.push("\n", Style::new());
                }
                Token::Tag {
                    name,
                    attrs,
                    closing,
                    ..
                } if name == "img" && !closing => {
                    self.ensure_title();
                    self.img(&attrs);
                }
                Token::Tag {
                    name,
                    attrs,
                    closing,
                    ..
                } if name == "details" => {
                    if closing {
                        self.close_details();
                    } else {
                        self.open_details(attrs.iter().any(|(k, _)| k == "open"));
                    }
                }
                Token::Tag { name, closing, .. } if name == "summary" => {
                    if closing {
                        self.ensure_title();
                    } else if let Some(d) = self.details.last_mut().filter(|d| d.body.is_none()) {
                        d.summary = Some(String::new());
                    }
                }
                Token::Tag {
                    name,
                    attrs,
                    closing,
                    raw: text,
                } => {
                    if !self.common_tag(&name, &attrs, closing) {
                        self.ensure_title();
                        self.push(text, Style::new());
                        raw = true;
                    }
                }
                Token::Text(text) => {
                    let summary = self.details.last_mut().and_then(|d| d.summary.as_mut());
                    match summary {
                        Some(summary) => summary.push_str(text),
                        None if text.trim().is_empty() => {}
                        None => {
                            self.ensure_title();
                            let text = self.scripted(text).into_owned();
                            self.push(&text, Style::new());
                            raw = true;
                        }
                    }
                }
            }
        }
        if block && raw && !self.pieces.is_empty() {
            self.push("\n", Style::new());
        }
    }

    /// 有正文进来了：套着的 `<details>` 还没画标题的，先画标题（没写 `<summary>` 的用默认的字）。
    /// 除了 HTML 的事件，每个事件进来都先过一遍。
    pub(super) fn content_arrives(&mut self, event: &Event) {
        if !matches!(event, Event::Html(_) | Event::InlineHtml(_)) {
            self.ensure_title();
        }
    }

    /// 回答排完：还没收尾的 `<details>`（还在流）照开关收尾，收着的里面不画。
    pub(super) fn close_all_details(&mut self) {
        while !self.details.is_empty() {
            self.close_details();
        }
    }

    /// `<svg` 开头的 HTML：从原文里这一处找到 `</svg>`，整段收成一张图，里面的事件跳过；还没到 `</svg>` 的
    /// （回答还在流）照代码块写到结尾。交回真表示收走了。
    pub(super) fn svg(&mut self, text: &str, event: &Event, at: usize) -> bool {
        let (Event::Html(html) | Event::InlineHtml(html)) = event else {
            return false;
        };
        if !html.trim_start().to_ascii_lowercase().starts_with("<svg") {
            return false;
        }
        self.ensure_title();
        self.flush();
        if matches!(event, Event::Html(_)) {
            self.gap();
        }
        let start = at + find(&text[at..], "<svg").unwrap_or(0);
        let end = find(&text[start..], "</svg>").map(|i| start + i + "</svg>".len());
        let source = &text[start..end.unwrap_or(text.len())];
        if end.is_some() {
            let fallback = self.aside(|r| r.code_lines("svg", source));
            self.figure(FigureKind::Svg, source.to_string(), fallback, (None, None));
        } else {
            self.code_lines("svg", source);
        }
        self.skip = Some((at, end.unwrap_or(text.len())));
        true
    }

    /// 这个事件在收走的 `<svg>` 那一截里：跳过。收尾的事件不跳（段落、块照常收）。
    pub(super) fn skips(&mut self, event: &Event, at: usize) -> bool {
        let Some((from, to)) = self.skip else {
            return false;
        };
        if at >= to {
            self.skip = None;
            return false;
        }
        at >= from && !matches!(event, Event::End(_))
    }

    /// `<img>`：和 `![alt](src)` 一样写，本机的记下来接着画，带着写的宽高。没写 `src` 的不管。
    fn img(&mut self, attrs: &[(String, String)]) {
        let get = |key: &str| {
            attrs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        let Some(src) = get("src") else {
            return;
        };
        let pixels = |key: &str| get(key).and_then(|v| v.trim_end_matches("px").parse().ok());
        self.styles.push(theme::md_image());
        self.link_starts.push((src.to_string(), self.pieces.len()));
        let label = self.labels.image.clone();
        self.push(&label, Style::new());
        self.push(get("alt").unwrap_or_default(), Style::new());
        self.end_link(true);
        if let Some(last) = self.images.last_mut().filter(|(url, _)| url == src) {
            last.1 = (pixels("width"), pixels("height"));
        }
    }

    fn open_details(&mut self, open: bool) {
        self.ensure_title();
        self.flush();
        self.gap();
        let index = self.details_seen;
        self.details_seen += 1;
        self.details.push(Details {
            index,
            open: open != self.flipped.contains(&index),
            summary: None,
            body: None,
        });
    }

    fn close_details(&mut self) {
        if self.details.is_empty() {
            return;
        }
        self.ensure_title();
        self.flush();
        let Some(details) = self.details.pop() else {
            return;
        };
        if let Some(at) = self
            .containers
            .iter()
            .rposition(|c| matches!(c, Container::Details))
        {
            self.containers.remove(at);
        }
        if let (false, Some(body)) = (details.open, details.body) {
            self.out.truncate(body);
        }
        self.just_titled = false;
    }

    /// 最里面那一块 `<details>` 还没画标题的，画上：记号（`markdown.details_marks`）算引子，字加粗，整行能点。
    fn ensure_title(&mut self) {
        let Some(details) = self.details.last_mut().filter(|d| d.body.is_none()) else {
            return;
        };
        let written = details.summary.take().unwrap_or_default();
        let (index, open) = (details.index, details.open);
        let text = match written.trim() {
            "" => self.labels.details.clone(),
            text => text.to_string(),
        };
        self.flush();
        let marker = self.labels.details_marks[usize::from(open)].clone();
        let bold = Style::new().add_modifier(Modifier::BOLD);
        let room = self.room().saturating_sub(2).max(1);
        for (i, folded) in fold(&[Piece::new(text, bold)], room)
            .into_iter()
            .enumerate()
        {
            let mut lead = self.lead(i == 0);
            let mark = if i == 0 {
                marker.clone()
            } else {
                " ".repeat(marker.width())
            };
            lead.push(Span::styled(mark, theme::md_list()));
            self.out.push(MdLine {
                lead,
                folded,
                copy: true,
                figure: None,
                details: Some(index),
                card: None,
            });
        }
        if let Some(details) = self.details.last_mut() {
            details.body = Some(self.out.len());
        }
        self.containers.push(Container::Details);
        self.just_titled = true;
    }
}

mod scan;
mod styles;

pub(super) use styles::Opened;

#[cfg(test)]
mod tests;
