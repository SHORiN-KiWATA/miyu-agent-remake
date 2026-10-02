//! 自成一块的几样：代码块（蓝图 `tui.md`「她的回答：Markdown」第 7 条），和要画成图的图片、
//! 块级公式、mermaid（「图片、公式和 mermaid 图」）。
//!
//! 要画成图的，这里只排出一行「这里有张图」，带着源码和画不成图时写的行；画不画、画成几行，
//! 由画正文的那一层照终端和做好的图定（`ui/figure_rows.rs`）。

use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::inline::{Folded, Piece};
use super::{MdLine, Renderer, code, math};
use crate::theme;

/// `<img>` 写的宽、高（像素），没写的是 `None`。
pub type Size = (Option<u32>, Option<u32>);

/// 要画成图的是哪一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FigureKind {
    /// 本机的图片文件，源码是写着的地址。
    Image,
    /// mermaid 图，源码是图的定义。
    Mermaid,
    /// 块级公式，源码是 LaTeX。
    Math,
    /// 回答里写的 `<svg>…</svg>`，源码就是它。
    Svg,
}

/// 一张要画成图的东西。
#[derive(Debug, Clone)]
pub struct Figure {
    /// 哪一种。
    pub kind: FigureKind,
    /// 源码：图片的地址、mermaid 的定义、公式的 LaTeX。
    pub source: String,
    /// 画不成图时写的行：mermaid、SVG 是代码块，公式是一行 Unicode，图片没有（那一行 `[图片: …]` 已经写了）。
    pub fallback: Vec<MdLine>,
    /// `<img>` 写的宽（像素）；没写的照图本身。
    pub width: Option<u32>,
    /// `<img>` 写的高（像素）。
    pub height: Option<u32>,
}

/// 围栏代码块收完了没有：`block` 是它在原文里的那一截，最后一个非空行是一排（至少三个）``` 或 ~~~，
/// 而且不是开头那一行。
pub(super) fn closed(block: &str) -> bool {
    let mut lines = block.trim_end().lines();
    let opened = lines.next().is_some();
    let fence =
        |t: &str| t.len() >= 3 && (t.chars().all(|c| c == '`') || t.chars().all(|c| c == '~'));
    opened && lines.last().is_some_and(|l| fence(l.trim()))
}

impl Renderer<'_> {
    /// 代码块：上下两条框线，代码按语言着色，太长的折行。语言写 `mermaid` 的、收完了的另排成一张图，
    /// 这个代码块当它画不成图时的样子；没收完的（回答还在流）照代码写，不去画半截的图。
    pub(super) fn code_block(&mut self) {
        let Some((lang, body)) = self.code.take() else {
            return;
        };
        if lang.eq_ignore_ascii_case("mermaid") && self.code_closed {
            let fallback = self.aside(|r| r.code_lines(&lang, &body));
            self.figure(FigureKind::Mermaid, body, fallback, (None, None));
        } else {
            self.code_lines(&lang, &body);
        }
    }

    /// 块级公式：先把前面收着的字排出去，自己占一块，画不成图时写成 Unicode：有分式的上下摞（`stack.rs`），
    /// 摞出来比正文宽的照一行写（折开就对不齐了）。
    pub(super) fn display_math(&mut self, tex: &str) {
        self.flush();
        let lines = super::stack::stacked(tex, self.math);
        let room = usize::from(self.room());
        let fits = lines
            .iter()
            .all(|l| unicode_width::UnicodeWidthStr::width(l.as_str()) <= room);
        let text = if lines.len() > 1 && fits {
            lines.join("\n")
        } else {
            math::unicode(tex, self.math)
        };
        let fallback = self.aside(|r| r.emit(vec![Piece::new(text, theme::md_math())], true));
        self.figure(
            FigureKind::Math,
            tex.trim().to_string(),
            fallback,
            (None, None),
        );
    }

    /// 行内公式：转成 Unicode，公式色。
    pub(super) fn inline_math(&mut self, tex: &str) {
        let text = math::unicode(tex, self.math);
        self.push(&text, theme::md_math());
    }

    /// 一张图片收完：本机的记下来，这一段排完以后接着画（网上的不下载，只有 `[图片: …]` 那一行）。
    pub(super) fn image(&mut self, url: &str) {
        let remote = ["http://", "https://", "data:"]
            .iter()
            .any(|scheme| url.starts_with(scheme));
        if !remote && self.table.is_none() {
            self.images.push((url.to_string(), (None, None)));
        }
    }

    /// 这一段里的图片排在这一段后面。
    pub(super) fn flush_images(&mut self) {
        for (url, size) in std::mem::take(&mut self.images) {
            self.figure(FigureKind::Image, url, Vec::new(), size);
        }
    }

    /// 一行「这里有张图」：`size` 是 `<img>` 写的宽高。
    pub(super) fn figure(
        &mut self,
        kind: FigureKind,
        source: String,
        fallback: Vec<MdLine>,
        (width, height): Size,
    ) {
        let lead = self.lead(false);
        self.out.push(MdLine {
            lead,
            folded: Folded::default(),
            copy: false,
            figure: Some(Figure {
                kind,
                source,
                fallback,
                width,
                height,
            }),
            details: None,
            card: None,
        });
    }

    /// 照常排，但排出来的行不放进正文，交回来。
    pub(super) fn aside(&mut self, draw: impl FnOnce(&mut Self)) -> Vec<MdLine> {
        let before = self.out.len();
        draw(self);
        self.out.split_off(before)
    }

    pub(super) fn code_lines(&mut self, lang: &str, body: &str) {
        let room = usize::from(self.room());
        let label = if lang.is_empty() {
            "╭─ code ".to_string()
        } else {
            format!("╭─ code {lang} ")
        };
        let top = format!("{label}{}", "─".repeat(room.saturating_sub(label.width())));
        self.frame(top);
        // 一个代码块一个着色的：块注释、三引号字符串接到下一行（`tui.md`「代码着色」第 2 条）。
        let mut colors = code::Highlighter::new(self.languages.find(lang));
        for line in body.trim_end_matches('\n').split('\n') {
            let pieces = colors.line(line);
            let pieces = if pieces.is_empty() {
                vec![Piece::new(" ", Style::new())]
            } else {
                pieces
            };
            self.emit(pieces, true);
        }
        self.frame("─".repeat(room));
    }

    fn frame(&mut self, text: String) {
        let lead = self.lead(false);
        let folded = Folded {
            spans: vec![Span::styled(text, theme::dim())],
            links: Vec::new(),
            joined: false,
        };
        self.out.push(MdLine {
            lead,
            folded,
            copy: false,
            figure: None,
            details: None,
            card: None,
        });
    }
}
