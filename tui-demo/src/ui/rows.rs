//! 把正文排成一行一行：每一行记着画什么、点它是什么、能复制的字从哪一列起。
//!
//! 行首的缩进和两格槽（你说的话的 `┃`、转圈）不算内容，复制时不带（`13-终端界面.md` 第六节）。

use crate::human::Human;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::{done_row, figure_rows, job_rows, timeline};
use std::cell::RefCell;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::config::Config;
use crate::core::Level;
use crate::figures::Figures;
use crate::input::pieces;
use crate::markdown::{self, MdLine};
use crate::theme;
use crate::transcript::{Entry, Kind};

/// 点一行时点中的东西。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// 时间线的一段（第几条正文）：展开、收起。
    Segment(usize),
    /// 一段里的一步：点开、收起。
    Step(usize, usize),
    /// 正文的某一条（撤销那一行）：点开、收起。
    Entry(usize),
    /// 她的回答（第几条正文）里第几个 `<details>`：展开、收起（蓝图「她的回答：Markdown」第 15 条）。
    Details(usize, usize),
}

/// 排好的一行。
#[derive(Debug, Clone)]
pub struct Row {
    /// 画出来的样子，从正文区的左边算起。
    pub line: Line<'static>,
    /// 点它点中的东西。
    pub target: Option<Target>,
    /// 整行铺底色：点开的步骤。
    pub shade: bool,
    /// 内容的字，复制用。
    pub plain: String,
    /// 收起的块在这一行占的列和原文，复制时替换；原文跨折行共享。
    pub copy_blocks: Vec<CopyBlock>,
    /// 内容从正文区左边第几列起。
    pub content_x: u16,
    /// 这一行是上一行折下来的：复制时接回上一行，不加换行。
    pub joined: bool,
    /// 链接：从内容开头算的起列、止列（不含）、地址。
    pub links: Vec<(u16, u16, String)>,
    /// 复制时带不带这一行：代码块的框线不带。
    pub copy: bool,
    /// 这一行是一张图的第几行（蓝图「图片、公式和 mermaid 图」第 2 条）。
    pub figure: Option<FigureCell>,
    /// 同一行的第二张图（链接卡片标题前面的网站图标）。
    pub icon: Option<FigureCell>,
    /// 这一行是「正在画图」：图做好了要重排（按条记着的行认它，「正文」第 8 条）。
    pub figure_pending: bool,
}

/// 收起的块的一截：显示列映射到整块原文；同一条目的同一块只复制一次。
#[derive(Debug, Clone)]
pub struct CopyBlock {
    /// 条目和块的编号；同名块也各是各的。
    pub id: (usize, usize),
    /// 这一行内容里的起列和止列（不含）。
    pub cols: (u16, u16),
    /// 整块原文或文件路径；折行只共享，不重复存全文。
    pub text: std::rc::Rc<str>,
}

/// 图的一行：哪张图（做好的图的键）的第几行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FigureCell {
    /// 做好的图的键（`Figures::get`）。
    pub key: u64,
    /// 图的第几行。
    pub row: u16,
    /// 从内容的第几列画起（链接卡片的网站图标在封面图右边）。
    pub x: u16,
}

/// 排版要的东西。
pub struct Ctx<'a> {
    /// 配置。
    pub config: &'a Config,
    /// 工具的显示名（仓库 `resources/software/basesystem/human/`）。
    pub human: &'a Human,
    /// 内容前面的缩进：正文区左边到两格槽。
    pub indent: String,
    /// 内容多宽，和输入框里的字一样宽。
    pub width: u16,
    /// 鼠标悬在哪。
    pub hover: Option<Target>,
    /// 转圈转到第几帧。
    pub frame: usize,
    /// 回答排好的行，按「这一条的字、宽度」缓存。
    pub md: &'a RefCell<MdCache>,
    /// 做好的图；没有的交给后台做。
    pub figures: &'a RefCell<Figures>,
    /// 链接卡片的账：没要过的卡片、图记进单子（`link_cards.rs`）。
    pub cards: &'a RefCell<crate::link_cards::LinkCards>,
    /// mermaid 图的账：没问过核心的记进单子（`diagrams.rs`）。
    pub diagrams: &'a RefCell<crate::diagrams::Diagrams>,
    /// 她正在写的那一条回答：写完了才换卡片（蓝图「链接卡片」第 1 条）。
    pub writing: Option<u64>,
    /// 现在的权限级别：你说的话没记着级别的（不该有），竖线照它上色。
    pub level: Level,
    /// 窗口有几行高：图最多占它的几分之几（蓝图「图片、公式和 mermaid 图」第 3 条）。
    pub screen_rows: u16,
}

impl Ctx<'_> {
    /// 内容从正文区左边第几列起：缩进加两格槽。
    pub fn content_x(&self) -> u16 {
        u16::try_from(self.indent.width() + 2).unwrap_or(u16::MAX)
    }

    /// 一行：缩进、两格槽、内容。
    pub fn row(&self, slot: Span<'static>, content: Vec<Span<'static>>) -> Row {
        self.led_row(slot, Vec::new(), content)
    }

    /// 一行：缩进、两格槽、引子、内容。引子（竖线 `│ `、缩进）画出来，复制时不带。
    pub fn led_row(
        &self,
        slot: Span<'static>,
        lead: Vec<Span<'static>>,
        content: Vec<Span<'static>>,
    ) -> Row {
        let plain = content.iter().map(|s| s.content.as_ref()).collect();
        let lead_width: usize = lead.iter().map(Span::width).sum();
        let mut spans = vec![Span::raw(self.indent.clone()), slot];
        spans.extend(lead);
        spans.extend(content);
        Row {
            line: Line::from(spans),
            target: None,
            shade: false,
            plain,
            copy_blocks: Vec::new(),
            content_x: self.content_x() + u16::try_from(lead_width).unwrap_or(0),
            joined: false,
            links: Vec::new(),
            copy: true,
            figure: None,
            icon: None,
            figure_pending: false,
        }
    }

    /// 空的两格槽。
    pub fn blank_slot(&self) -> Span<'static> {
        Span::raw("  ")
    }
}

/// 正文第 `i` 条排成的行（不带前后的空行）。
pub fn entry_rows(i: usize, entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    match (&entry.segment, entry.kind.clone()) {
        (Some(segment), _) => timeline::rows(i, segment, ctx),
        (None, Kind::Undo) => super::undo_rows::rows(i, entry, ctx),
        (None, Kind::Job) => job_rows::rows(i, entry, ctx),
        (None, Kind::Reply) => reply_rows(i, entry, ctx),
        (None, Kind::User) => super::user_rows::rows(i, entry, ctx),
        (None, _) if entry.progress.is_some() => super::compaction_rows::rows(entry, ctx),
        (None, _) => text_rows(entry, ctx),
    }
}

/// 这一条画不画：藏起来的、排着队的、空的时间线段、空的字不画。
pub fn shown(entry: &Entry) -> bool {
    !entry.hidden
        && !entry.queued
        && match &entry.segment {
            Some(segment) => !segment.steps.is_empty(),
            None => entry.kind == Kind::Undo || !entry.text.trim().is_empty(),
        }
}

/// 太长就截掉，末尾写 `…`，不折行（`13-终端界面.md` 第三节第 1 条）。
pub fn clip(text: &str, width: u16) -> String {
    let width = usize::from(width);
    if text.width() <= width {
        return text.to_string();
    }
    let mut out = String::new();
    for c in text.chars() {
        if out.width() + c.to_string().width() + 1 > width {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// 缓存认的键：字、点过的 `<details>`、换过几次主题的哈希（颜色烤在排好的行里，换了主题要重排）。
fn cache_key(text: &str, details: &[usize], language: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    (text, details, language).hash(&mut hasher);
    theme::generation().hash(&mut hasher);
    hasher.finish()
}

pub use super::md_cache::MdCache;

/// 她的回答：按 Markdown 排（蓝图 `tui.md`「她的回答：Markdown」），查缓存。
fn reply_rows(index: usize, entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let text = entry.text.trim_matches('\n');
    let hash = cache_key(text, &entry.details, ctx.config.language.code());
    let lines = ctx.md.borrow_mut().lines(index, hash, ctx.width, || {
        let kit = markdown::Kit {
            languages: &ctx.config.languages,
            math: &ctx.config.math,
            labels: &ctx.config.text.markdown,
        };
        markdown::render(text, ctx.width, &kit, &entry.details)
    });
    // 写完了的回答里独占一行的链接：核心交回了卡片的换成卡片（蓝图「链接卡片」第 1 条）。
    let cards = ctx.writing != Some(entry.id);
    let mut out: Vec<Row> = Vec::new();
    let mut last_card: Option<String> = None;
    // 卡片前后各空一行，已经有空行的不再补（同一天项目主人报：上下没有空行）。
    let mut gap_after = false;
    for line in lines {
        let card = line
            .card
            .clone()
            .filter(|_| cards)
            .and_then(|url| ctx.cards.borrow_mut().card(&url).cloned().map(|c| (url, c)));
        match card {
            // 一个链接折成好几行的：卡片只画一次，别的行不要。
            Some((url, _)) if last_card.as_deref() == Some(url.as_str()) => {}
            Some((url, card)) => {
                if out.last().is_some_and(|r| !super::link_card::blank(r)) {
                    out.push(ctx.row(ctx.blank_slot(), Vec::new()));
                }
                gap_after = true;
                out.extend(super::link_card::rows(
                    &ctx.blank_slot(),
                    &line.lead,
                    &url,
                    &card,
                    ctx,
                ));
                last_card = Some(url);
            }
            None => {
                last_card = None;
                let rows = match &line.figure {
                    Some(figure) => figure_rows::rows(line.lead.clone(), figure, ctx),
                    None => vec![details_row(index, line, ctx)],
                };
                if std::mem::take(&mut gap_after)
                    && rows.first().is_some_and(|r| !super::link_card::blank(r))
                {
                    out.push(ctx.row(ctx.blank_slot(), Vec::new()));
                }
                out.extend(rows);
            }
        }
    }
    out
}

/// 一行 Markdown；是 `<details>` 标题的，整行能点，悬停变亮（蓝图「她的回答：Markdown」第 15 条）。
fn details_row(index: usize, line: MdLine, ctx: &Ctx) -> Row {
    let Some(k) = line.details else {
        return md_row(line, ctx);
    };
    let target = Target::Details(index, k);
    let mut row = md_row(line, ctx);
    row.target = Some(target);
    if ctx.hover == Some(target) {
        for span in &mut row.line.spans {
            span.style = span.style.patch(theme::hover());
        }
    }
    row
}

/// 排好的一行 Markdown 放进正文。
pub(super) fn md_row(line: MdLine, ctx: &Ctx) -> Row {
    let mut row = ctx.led_row(ctx.blank_slot(), line.lead, line.folded.spans);
    row.joined = line.folded.joined;
    row.links = line.folded.links;
    row.copy = line.copy;
    row
}

/// 一条字：旁白、出错、收尾行、答完的引用块，平铺；折行按显示宽度。你说的话在 `user_rows.rs`。
fn text_rows(entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let layout = &ctx.config.layout;
    let (slot, style) = match entry.kind {
        Kind::Note | Kind::Done => (ctx.blank_slot(), theme::dim()),
        Kind::Answered => (
            Span::styled(layout.user_bar.clone(), theme::dim()),
            theme::dim(),
        ),
        Kind::Recap => (ctx.blank_slot(), theme::dim()),
        Kind::Error | Kind::Cut => (ctx.blank_slot(), theme::error()),
        Kind::User | Kind::Reply | Kind::Steps | Kind::Undo | Kind::Job => {
            (ctx.blank_slot(), Style::new())
        }
    };
    // 前面带绿色记号的（压好了的 `● `）：记号是引子，折下来的行和字对齐（「正文」第 9 条）。回顾的 `※` 一样，暗色（「回顾」第 2 条）。
    let (mark, mark_style) = match entry.kind {
        Kind::Recap => (layout.recap_mark.as_str(), theme::dim()),
        _ => (entry.mark.as_deref().unwrap_or_default(), theme::good()),
    };
    let mark_width = u16::try_from(mark.width()).unwrap_or(0);
    let width = ctx.width.saturating_sub(mark_width).max(1);
    let pieces = match entry.kind {
        // 收尾行只在 ` · ` 处折（「窗口小的时候」第 5 条）。
        Kind::Done | Kind::Cut => {
            done_row::pieces(&done_row::line(entry.level, &entry.text, layout), width)
        }
        // 模型的回答常以换行开头、结尾，前后的空行不画。
        _ => pieces(entry.text.trim_matches('\n'), width),
    };
    pieces
        .into_iter()
        .enumerate()
        .map(|(i, (piece, joined))| {
            let lead = match (mark.is_empty(), i) {
                (true, _) => Vec::new(),
                (false, 0) => vec![Span::styled(mark.to_string(), mark_style)],
                (false, _) => vec![Span::raw(" ".repeat(usize::from(mark_width)))],
            };
            let mut row = ctx.led_row(slot.clone(), lead, vec![Span::styled(piece, style)]);
            row.joined = joined;
            row
        })
        .collect()
}

#[cfg(test)]
mod tests;
