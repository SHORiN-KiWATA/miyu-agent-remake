//! 她的回答按 Markdown 画（蓝图 `tui.md`「她的回答：Markdown」）：每次画都拿整段交给 pulldown-cmark，
//! 走一遍解析出的事件，排成一行行。行首的列表记号、引用的竖线是引子，复制时不带；代码块的框线整行不复制。
//!
//! 视图投影做出来以前，解析暂时在头里（蓝图「还没有的」）。

mod blocks;
mod code;
mod html;
mod inline;
mod kit;
mod layout;
mod links;
mod math;
mod table;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

pub use blocks::{Figure, FigureKind, Size};
pub use code::Languages;
pub use inline::Folded;
use inline::{Piece, fold};
pub use kit::{Kit, Labels};
pub use math::Math;
use table::Cell;

use crate::theme;

/// 画好的一行。
#[derive(Debug, Clone)]
pub struct MdLine {
    /// 引子：列表记号、引用的竖线。画出来，复制时不带。
    pub lead: Vec<Span<'static>>,
    /// 内容。
    pub folded: Folded,
    /// 复制时带不带这一行：代码块的框线不带。
    pub copy: bool,
    /// 这一行是一张要画成图的东西（图片、块级公式、mermaid、SVG）；内容是空的。
    pub figure: Option<Figure>,
    /// 这一行是第几个 `<details>` 的标题：点它展开、收起。
    pub details: Option<usize>,
}

/// 把一段 Markdown 排成 `width` 列宽的行，照 `kit` 着色、转写公式、写字。`flipped` 是这一条回答里点过的
/// `<details>`（第几个），和它写的 `open` 反过来。
pub fn render(text: &str, width: u16, kit: &Kit, flipped: &[usize]) -> Vec<MdLine> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH;
    let mut r = Renderer {
        width,
        languages: kit.languages,
        math: kit.math,
        labels: kit.labels,
        flipped,
        details: Vec::new(),
        details_seen: 0,
        just_titled: false,
        html_open: Vec::new(),
        skip: None,
        images: Vec::new(),
        code_closed: false,
        out: Vec::new(),
        containers: Vec::new(),
        lists: Vec::new(),
        styles: Vec::new(),
        link_starts: Vec::new(),
        pieces: Vec::new(),
        code: None,
        table: None,
        fresh_item: false,
    };
    for (event, range) in Parser::new_ext(text, options).into_offset_iter() {
        if r.skips(&event, range.start) || r.svg(text, &event, range.start) {
            continue;
        }
        if matches!(event, Event::End(TagEnd::CodeBlock)) {
            r.code_closed = blocks::closed(&text[range]);
        }
        r.event(event);
    }
    r.finish()
}

/// 套在外面的一层：引用、列表的一项，或者展开着的 `<details>`。
enum Container {
    Quote,
    Item { marker: String, used: bool },
    Details,
}

/// 正在收的表格。
struct TableState {
    aligns: Vec<Alignment>,
    head: Vec<Cell>,
    rows: Vec<Vec<Cell>>,
    row: Vec<Cell>,
}

struct Renderer<'a> {
    width: u16,
    languages: &'a Languages,
    math: &'a Math,
    labels: &'a Labels,
    /// 这一条回答里点过的 `<details>`（第几个）。
    flipped: &'a [usize],
    /// 套着的 `<details>`，外层在前。
    details: Vec<html::Details>,
    /// 见过几个 `<details>`：下一个的编号。
    details_seen: usize,
    /// 刚画了 `<details>` 的标题，里面还没有内容：紧接着的内容前面不空行。
    just_titled: bool,
    /// 开着的行内 HTML 标签（`<b>`、`<a href>`、`<sup>` 这些）：名字、叠了什么。
    html_open: Vec<(String, html::Opened)>,
    /// 收走的 `<svg>` 在原文里的范围：里面的事件跳过。
    skip: Option<(usize, usize)>,
    /// 这一段里本机图片的地址和写的宽高（`<img>` 才有）：这一段排完接着画。
    images: Vec<(String, Size)>,
    out: Vec<MdLine>,
    containers: Vec<Container>,
    /// 每一层列表：有序的记着下一个数，无序的是 `None`。
    lists: Vec<Option<u64>>,
    /// 行内样子，一层叠一层。
    styles: Vec<Style>,
    /// 链接开头时：地址、那时候收了几段。
    link_starts: Vec<(String, usize)>,
    /// 这一块收着的行内片段。
    pieces: Vec<Piece>,
    /// 在收的代码块：语言、正文。
    code: Option<(String, String)>,
    /// 刚收完的代码块在原文里有没有收尾的那一行（回答还在流时，最后一块可能还没收完）。
    code_closed: bool,
    table: Option<TableState>,
    /// 列表这一项的第一段：前面不空行。
    fresh_item: bool,
}

impl Renderer<'_> {
    fn event(&mut self, event: Event) {
        self.content_arrives(&event);
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => match &mut self.code {
                Some((_, body)) => body.push_str(&text),
                None => self.text(&text),
            },
            Event::Code(text) => self.push(&text, theme::md_code()),
            Event::SoftBreak | Event::HardBreak => self.line_break(),
            Event::Html(html) => self.html(&html, true),
            Event::InlineHtml(html) => self.html(&html, false),
            Event::Rule => self.rule(),
            Event::TaskListMarker(done) => {
                if let Some(Container::Item {
                    marker,
                    used: false,
                }) = self.containers.last_mut()
                {
                    *marker = if done { "☑ " } else { "☐ " }.to_string();
                }
            }
            Event::InlineMath(tex) => self.inline_math(&tex),
            Event::DisplayMath(tex) => self.display_math(&tex),
            Event::FootnoteReference(name) => self.push(&format!("[^{name}]"), Style::new()),
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                if !std::mem::take(&mut self.fresh_item) {
                    self.flush();
                    self.gap();
                }
            }
            Tag::Heading { level, .. } => {
                self.flush();
                self.gap();
                // 不写 `#`；一级标题加下划线，别的级别一个样（蓝图「她的回答：Markdown」第 3 条）。
                let style = if level == HeadingLevel::H1 {
                    theme::md_heading().add_modifier(Modifier::UNDERLINED)
                } else {
                    theme::md_heading()
                };
                self.styles.push(style);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.gap();
                self.containers.push(Container::Quote);
            }
            Tag::CodeBlock(kind) => {
                self.flush();
                self.gap();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((lang, String::new()));
            }
            Tag::List(start) => {
                self.flush();
                if self.lists.is_empty() {
                    self.gap();
                }
                self.lists.push(start);
            }
            Tag::Item => {
                self.flush();
                let depth = self.lists.len();
                let marker = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => format!("{} ", self.labels.bullets[depth.saturating_sub(1).min(2)]),
                };
                self.containers.push(Container::Item {
                    marker,
                    used: false,
                });
                self.fresh_item = true;
            }
            Tag::Emphasis => self.styles.push(theme::md_italic()),
            Tag::Strong => self.styles.push(theme::md_bold()),
            Tag::Strikethrough => self.styles.push(Style::new().crossed_out()),
            Tag::Link { dest_url, .. } => {
                self.styles.push(theme::md_link());
                self.link_starts
                    .push((dest_url.to_string(), self.pieces.len()));
            }
            Tag::Image { dest_url, .. } => {
                self.styles.push(theme::md_image());
                // 先记起点再写「[图片: 」：整行一条链接，悬停时下划线从头连到尾（蓝图第 9、12 条）。
                self.link_starts
                    .push((dest_url.to_string(), self.pieces.len()));
                let label = self.labels.image.clone();
                self.push(&label, Style::new());
            }
            Tag::Table(aligns) => {
                self.flush();
                self.gap();
                self.table = Some(TableState {
                    aligns,
                    head: Vec::new(),
                    rows: Vec::new(),
                    row: Vec::new(),
                });
            }
            Tag::TableCell => self.pieces.clear(),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::Heading(_) => {
                self.styles.pop();
                self.flush();
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.containers.pop();
            }
            TagEnd::CodeBlock => self.code_block(),
            TagEnd::HtmlBlock => self.flush(),
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::Item => {
                self.flush();
                self.containers.pop();
                self.fresh_item = false;
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.styles.pop();
            }
            TagEnd::Link => self.end_link(false),
            TagEnd::Image => self.end_link(true),
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.pieces);
                if let Some(t) = &mut self.table {
                    t.row.push(cell);
                }
            }
            TagEnd::TableHead => {
                if let Some(t) = &mut self.table {
                    t.head = std::mem::take(&mut t.row);
                }
            }
            TagEnd::TableRow => {
                if let Some(t) = &mut self.table {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    let lines = table::render(&t.head, &t.rows, &t.aligns, self.room());
                    for folded in lines {
                        let lead = self.lead(true);
                        self.out.push(MdLine {
                            lead,
                            folded,
                            copy: true,
                            figure: None,
                            details: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    /// 链接收完：标题成链；标题和地址不一样的，后面跟 ` <地址>`。图片写成 `[图片: 说明] <地址>`。
    fn end_link(&mut self, image: bool) {
        self.styles.pop();
        let Some((url, start)) = self.link_starts.pop() else {
            return;
        };
        let label: String = self.pieces[start..]
            .iter()
            .map(|p| p.text.as_str())
            .collect();
        for piece in &mut self.pieces[start..] {
            piece.link = Some(url.clone());
        }
        if image {
            self.pieces
                .push(Piece::linked("]", theme::md_image(), &url));
            self.image(&url);
        }
        if image || label.trim() != url {
            self.pieces.extend(links::address(&url));
        } else {
            // 只有地址的（`<地址>`）用地址色，和 `<地址>` 里那一截一个样（蓝图第 9 条）。
            let style = self.style().patch(theme::md_url());
            for piece in &mut self.pieces[start..] {
                piece.style = style;
            }
        }
    }

    /// 一段字：不在链接里的，认出裸地址；在 `<sub>`、`<sup>` 里的转成下标、上标。
    fn text(&mut self, text: &str) {
        let text = self.scripted(text).into_owned();
        let text = text.as_str();
        let style = self.style();
        if !self.link_starts.is_empty() {
            self.push(text, Style::new());
            return;
        }
        let mut at = 0;
        for (start, end) in links::bare_urls(text) {
            self.push(&text[at..start], Style::new());
            let url = &text[start..end];
            self.pieces
                .push(Piece::linked(url, style.patch(theme::md_url()), url));
            at = end;
        }
        self.push(&text[at..], Style::new());
    }

    /// 收一段字，样子叠在当前的行内样子上；在链接里的记着地址。
    fn push(&mut self, text: &str, extra: Style) {
        if text.is_empty() {
            return;
        }
        let style = self.style().patch(extra);
        let link = self.link_starts.last().map(|(url, _)| url.clone());
        self.pieces.push(Piece {
            text: text.to_string(),
            style,
            link,
        });
    }

    fn style(&self) -> Style {
        let quoted = self
            .containers
            .iter()
            .any(|c| matches!(c, Container::Quote));
        let base = if quoted {
            theme::md_quote()
        } else {
            Style::new()
        };
        self.styles.iter().fold(base, |s, next| s.patch(*next))
    }

    /// 一段话里换行。这一行里有本机的图的，先把这一行排出来、紧接着画它的图，再接着排这一段下面的行
    /// （蓝图「图片、公式和 mermaid 图」第 3 条：每张图画在自己那一行下面）。
    fn line_break(&mut self) {
        if self.images.is_empty() {
            self.push("\n", Style::new());
            return;
        }
        let pieces = std::mem::take(&mut self.pieces);
        if !pieces.is_empty() {
            self.emit(links::relink_title(pieces), true);
        }
        self.flush_images();
    }

    /// 把收着的行内片段排出来。独占一行的「标题 (地址)」改写成链接（蓝图第 9 条）：
    /// 段落、列表项都走这里，所以列表里的也认。
    fn flush(&mut self) {
        self.close_open_tags();
        let pieces = std::mem::take(&mut self.pieces);
        if !pieces.is_empty() {
            self.emit(links::relink_title(pieces), true);
        }
        self.flush_images();
    }

    fn emit(&mut self, pieces: Vec<Piece>, copy: bool) {
        for (i, folded) in fold(&pieces, self.room()).into_iter().enumerate() {
            let lead = self.lead(i == 0);
            self.out.push(MdLine {
                lead,
                folded,
                copy,
                figure: None,
                details: None,
            });
        }
    }

    fn finish(mut self) -> Vec<MdLine> {
        self.flush();
        self.close_all_details();
        while self.out.last().is_some_and(MdLine::is_blank) {
            self.out.pop();
        }
        self.out
    }
}

#[cfg(test)]
mod tests;
