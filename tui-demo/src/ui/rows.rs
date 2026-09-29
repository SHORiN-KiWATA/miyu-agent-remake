//! 把正文排成一行一行：每一行记着画什么、点它是什么、能复制的字从哪一列起。
//!
//! 行首的缩进和两格槽（你说的话的 `┃`、转圈）不算内容，复制时不带（`13-终端界面.md` 第六节）。

use miyu_store::human::Human;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::{figure_rows, job_rows, timeline};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::config::Config;
use crate::core::Level;
use crate::figures::Figures;
use crate::input::pieces;
use crate::markdown::{self, MdLine};
use crate::theme;
use crate::transcript::{Entry, Kind, undo_counts};

/// 点一行时点中的东西。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// 时间线的一段（第几条正文）：展开、收起。
    Segment(usize),
    /// 一段里的一步：点开、收起。
    Step(usize, usize),
    /// 正文的某一条（撤销那一行）：点开、收起。
    Entry(usize),
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
}

/// 图的一行：哪张图（做好的图的键）的第几行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FigureCell {
    /// 做好的图的键（`Figures::get`）。
    pub key: u64,
    /// 图的第几行。
    pub row: u16,
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
    /// 现在的权限级别：你说的话没记着级别的（不该有），竖线照它上色。
    pub level: Level,
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
            content_x: self.content_x() + u16::try_from(lead_width).unwrap_or(0),
            joined: false,
            links: Vec::new(),
            copy: true,
            figure: None,
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
        (None, Kind::Undo) => undo_rows(i, entry, ctx),
        (None, Kind::Job) => job_rows::rows(i, entry, ctx),
        (None, Kind::Reply) => reply_rows(i, entry, ctx),
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

/// 撤销那一行：`↶ 撤销了 1 轮 · Ctrl+R 恢复 · 那一句的预览…`，只占一行。点开铺底色：全文、改回几个文件那一行。
fn undo_rows(i: usize, entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let target = Target::Entry(i);
    let style = if ctx.hover == Some(target) {
        theme::hover()
    } else {
        theme::dim()
    };
    let text = &ctx.config.text;
    let turns = entry.undo.as_ref().map_or(0, |r| r.turns);
    let mut head = format!(
        "{}{}",
        ctx.config.layout.undo_icon,
        text.undone.replace("{turns}", &turns.to_string())
    );
    let peek: String = entry.text.split_whitespace().collect::<Vec<_>>().join(" ");
    if !peek.is_empty() {
        head.push_str(" · ");
        head.push_str(&peek);
    }
    let mut out = vec![ctx.row(
        ctx.blank_slot(),
        vec![Span::styled(clip(&head, ctx.width), style)],
    )];
    if entry.open {
        let width = ctx.width.saturating_sub(2).max(1);
        let blank = || ctx.row(ctx.blank_slot(), Vec::new());
        out.push(blank());
        for (piece, joined) in pieces(entry.text.trim(), width) {
            let mut row = ctx.led_row(
                ctx.blank_slot(),
                vec![Span::raw("  ")],
                vec![Span::raw(piece)],
            );
            row.joined = joined;
            out.push(row);
        }
        let counts = entry.undo.as_ref().and_then(|r| undo_counts(r, text));
        if let Some(counts) = counts {
            out.push(blank());
            out.push(ctx.led_row(
                ctx.blank_slot(),
                vec![Span::raw("  ")],
                vec![Span::styled(counts, theme::dim())],
            ));
        }
        out.push(blank());
        for row in &mut out {
            row.shade = true;
        }
    }
    for row in &mut out {
        row.target = Some(target);
    }
    out
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

/// 收尾行：图标，这个级别要多空的（`done_gap`，只有 `▣` 多空一格），再接字（`tui.md`「正文」第 4 条）。
fn done_line(level: Option<Level>, text: &str, layout: &crate::config::Layout) -> String {
    let gap = level
        .and_then(|l| layout.done_gap.get(&l))
        .map_or("", String::as_str);
    format!("{}{gap}{text}", done_mark(level, layout))
}

/// 收尾行打头的符号：这一轮开始时的权限级别的图标；没记着级别的用兜底的 `✻`（`tui.md`「正文」第 4 条）。
fn done_mark(level: Option<Level>, layout: &crate::config::Layout) -> &str {
    level
        .and_then(|l| layout.level_icons.get(&l))
        .unwrap_or(&layout.done_icon)
}

/// 缓存认的键：字和换过几次主题的哈希（颜色烤在排好的行里，换了主题要重排）。
fn cache_key(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    theme::generation().hash(&mut hasher);
    hasher.finish()
}

/// 回答排好的行的缓存：第几条 → （字的哈希、宽度、排好的行）。字、宽度没变的不重排，只有在收的那一条每帧重排。
pub type MdCache = HashMap<usize, (u64, u16, Vec<MdLine>)>;

/// 她的回答：按 Markdown 排（蓝图 `tui.md`「她的回答：Markdown」），查缓存。
fn reply_rows(index: usize, entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let text = entry.text.trim_matches('\n');
    let hash = cache_key(text);
    let mut cache = ctx.md.borrow_mut();
    let fresh = cache
        .get(&index)
        .is_some_and(|(h, w, _)| *h == hash && *w == ctx.width);
    if !fresh {
        let lines = markdown::render(text, ctx.width, &ctx.config.languages, &ctx.config.math);
        cache.insert(index, (hash, ctx.width, lines));
    }
    let lines = cache
        .get(&index)
        .map(|(_, _, l)| l.clone())
        .unwrap_or_default();
    lines
        .into_iter()
        .flat_map(|line| match &line.figure {
            Some(figure) => figure_rows::rows(line.lead.clone(), figure, ctx),
            None => vec![md_row(line, ctx)],
        })
        .collect()
}

/// 排好的一行 Markdown 放进正文。
pub(super) fn md_row(line: MdLine, ctx: &Ctx) -> Row {
    let mut row = ctx.led_row(ctx.blank_slot(), line.lead, line.folded.spans);
    row.joined = line.folded.joined;
    row.links = line.folded.links;
    row.copy = line.copy;
    row
}

/// 一条字：你说的话带竖线，别的平铺；折行按显示宽度。
fn text_rows(entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let layout = &ctx.config.layout;
    let (slot, style) = match entry.kind {
        Kind::User => (
            Span::styled(
                layout.user_bar.clone(),
                theme::user_bar(entry.level.unwrap_or(ctx.level)),
            ),
            Style::new(),
        ),
        Kind::Note | Kind::Done => (ctx.blank_slot(), theme::dim()),
        Kind::Answered => (
            Span::styled(layout.user_bar.clone(), theme::dim()),
            theme::dim(),
        ),
        Kind::Error => (ctx.blank_slot(), theme::error()),
        Kind::Reply | Kind::Steps | Kind::Undo | Kind::Job => (ctx.blank_slot(), Style::new()),
    };
    let text = match entry.kind {
        Kind::Done => done_line(entry.level, &entry.text, layout),
        // 模型的回答常以换行开头、结尾，前后的空行不画。
        _ => entry.text.trim_matches('\n').to_string(),
    };
    let mut rows: Vec<Row> = pieces(&text, ctx.width.max(1))
        .into_iter()
        .map(|(piece, joined)| {
            let mut row = ctx.row(slot.clone(), vec![Span::styled(piece, style)]);
            row.joined = joined;
            row
        })
        .collect();
    // 你说的话上下各多一行竖线，框成一块（`13-终端界面.md` 第三节第 7 条）。
    if entry.kind == Kind::User {
        rows.insert(0, ctx.row(slot.clone(), Vec::new()));
        rows.push(ctx.row(slot, Vec::new()));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::cache_key;
    use crate::theme;

    #[test]
    fn the_done_line_starts_with_its_level_icon() {
        use super::done_mark;
        use crate::config::Config;
        use crate::core::Level;
        let layout = Config::builtin().unwrap().layout;
        assert_eq!(done_mark(Some(Level::ReadOnly), &layout), "⏸ ");
        assert_eq!(done_mark(Some(Level::Workspace), &layout), "▣ ");
        assert_eq!(done_mark(None, &layout), "✻ ", "没记着级别的兜底");
        // 只有 `▣` 后面空两格，别的空一格。
        let line = |level| super::done_line(level, "03:44", &layout);
        assert_eq!(line(Some(Level::Workspace)), "▣  03:44");
        assert_eq!(line(Some(Level::Full)), "⏵⏵ 03:44");
        assert_eq!(line(Some(Level::ReadOnly)), "⏸ 03:44");
        assert_eq!(line(None), "✻ 03:44");
    }

    #[test]
    fn a_new_theme_redraws_cached_replies() {
        let before = cache_key("**粗**");
        // 设回同一套：颜色不变（不扰别的测试），但换过一次，排好的样子就作废。
        let palette = theme::builtin().unwrap().remove(0).1;
        theme::set(palette);
        assert_ne!(cache_key("**粗**"), before);
    }
}
