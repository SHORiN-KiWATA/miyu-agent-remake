//! 你说的话（蓝图 `tui.md`「正文」第 2 条）：行首竖线，上下各多一行只有竖线的空行，竖线的颜色是发出去那一刻的权限级别。
//! 里面有粘贴块的，块照输入框里的样子写（品红字、暗紫底），整条能点：点开原地把每一块换成全文，再点换回块；
//! 悬停时块亮一档，展开着的粘的那几段铺上块的底色。附件（`[图片 1]`）也照块写，不展开，点块用系统的程序打开那个文件。
//! 别处来的话（蓝图「别处来的话」）竖线暗色，第一行暗色写来处。

use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::rows::{CopyBlock, Ctx, Row, Target};
use crate::input::wrap_words;
use crate::theme;
use crate::transcript::{Chip, Entry};
use std::rc::Rc;

/// 排成的行。`i` 是它在正文里是第几条。
pub fn rows(i: usize, entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    // 别处来的话：一行暗色的来处和预览，点开换成全文（「别处来的话」）。
    if let Some(from) = &entry.from {
        return super::foreign_rows::rows(i, entry, from, ctx);
    }
    // 别处来的话：竖线暗色（「别处来的话」第 1 条）；你说的照发出去那一刻的权限级别上色。
    let bar_style = if entry.from.is_some() {
        theme::dim()
    } else {
        theme::user_bar(entry.level.unwrap_or(ctx.level))
    };
    let bar = Span::styled(ctx.config.layout.user_bar.clone(), bar_style);
    let said = entry.text.trim_matches('\n');
    // 有粘贴块的才能点开；只有附件、文件块的，点块以外的地方没反应（「输入框」第 12 条）。
    let clickable = entry.pasted.iter().any(Chip::expandable);
    let hovered = clickable && ctx.hover == Some(Target::Entry(i));
    let (text, pieces) = shaped(said, &entry.pasted, entry.open, hovered);
    let mut out = vec![ctx.row(bar.clone(), Vec::new())];
    // 来处写在竖线里第一行，暗色；复制时不带。
    if let Some(from) = &entry.from {
        let mut head = ctx.row(bar.clone(), vec![Span::styled(from.clone(), theme::dim())]);
        head.copy = false;
        out.push(head);
    }
    let mut prev_end = None;
    // 独占一行的地址：核心交回了卡片的换成卡片（蓝图「链接卡片」第 1 条）；折成好几行的只画一次。
    let lone = lone_lines(&text);
    let mut carded: Option<usize> = None;
    let mut gap_after = false;
    for line in wrap_words(&text, ctx.width.max(1)) {
        if let Some((start, url)) = lone
            .iter()
            .find(|(s, e, _)| *s <= line.start && line.end <= *e)
            .map(|(s, _, u)| (*s, u.clone()))
        {
            if carded == Some(start) {
                continue;
            }
            let card = ctx.cards.borrow_mut().card(&url).cloned();
            if let Some(card) = card {
                // 前后各空一行（只有竖线），已经有空行的不再补。
                if out.last().is_some_and(|r| !super::link_card::blank(r)) {
                    out.push(ctx.row(bar.clone(), Vec::new()));
                }
                gap_after = true;
                out.extend(super::link_card::rows(&bar, &[], &url, &card, ctx));
                carded = Some(start);
                prev_end = None;
                continue;
            }
        }
        let styled: Vec<(usize, usize, Style)> =
            pieces.iter().map(|p| (p.from, p.to, p.style)).collect();
        if std::mem::take(&mut gap_after) && line.start < line.end {
            out.push(ctx.row(bar.clone(), Vec::new()));
        }
        let mut row = ctx.row(bar.clone(), spans(&text, line.start, line.end, &styled));
        row.links = links(&text, line.start, line.end, &pieces);
        row.copy_blocks = copy_blocks(i, &text, line.start, line.end, &pieces);
        row.joined = prev_end == Some(line.start);
        prev_end = Some(line.end);
        out.push(row);
    }
    out.push(ctx.row(bar, Vec::new()));
    if clickable {
        for row in &mut out {
            row.target = Some(Target::Entry(i));
        }
    }
    out
}

/// 字里独占一行的地址：那一行的字节范围、地址。
fn lone_lines(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut at = 0;
    for line in text.split('\n') {
        if let Some(url) = crate::markdown::lone_url(line) {
            out.push((at, at + line.len(), url.to_string()));
        }
        at += line.len() + 1;
    }
    out
}

/// 排好的字里的一块：字节范围、样子；附件另带它的文件，点块打开。
struct Piece {
    from: usize,
    to: usize,
    style: Style,
    file: Option<String>,
    replacement: Option<(usize, Rc<str>)>,
}

/// 排成的字，和每一块。收着的块写块上的字，悬停时亮一档；点开了粘贴块原地换成全文，悬停时粘的那几段铺上块的
/// 底色；附件怎么都是块，是指向它的文件的链接（「输入框」第 12 条：不展开、底色不掉，点块用系统的程序打开）。
fn shaped(text: &str, chips: &[Chip], open: bool, hovered: bool) -> (String, Vec<Piece>) {
    let chip = if hovered {
        theme::chip_hover()
    } else {
        theme::chip()
    };
    let mut out = String::with_capacity(text.len());
    let mut pieces = Vec::new();
    let mut at = 0;
    for (index, ((start, end), c)) in block_ranges(text, chips).into_iter().zip(chips).enumerate() {
        out.push_str(&text[at..start]);
        let from = out.len();
        let file = c.file.as_ref().map(|f| f.display().to_string());
        if open && c.expandable() {
            out.push_str(c.full.trim_matches('\n'));
            if hovered {
                let style = theme::chip_ground();
                pieces.push(Piece {
                    from,
                    to: out.len(),
                    style,
                    file,
                    replacement: None,
                });
            }
        } else {
            out.push_str(&text[start..end]);
            let style = chip;
            pieces.push(Piece {
                from,
                to: out.len(),
                style,
                file,
                replacement: (!c.attachment()).then(|| (index, Rc::from(c.full.as_str()))),
            });
        }
        at = end;
    }
    out.push_str(&text[at..]);
    (out, pieces)
}

/// 把收起的块投影到折行的显示列，复制不依赖标签的文字；同一块跨行共用原文和编号。
fn copy_blocks(
    entry: usize,
    text: &str,
    start: usize,
    end: usize,
    pieces: &[Piece],
) -> Vec<CopyBlock> {
    let cols = |a: usize, b: usize| u16::try_from(text[a..b].width()).unwrap_or(u16::MAX);
    pieces
        .iter()
        .filter_map(|p| {
            let (index, full) = p.replacement.as_ref()?;
            let (s, e) = (p.from.max(start), p.to.min(end));
            if s >= e {
                return None;
            }
            let from = cols(start, s);
            Some(CopyBlock {
                id: (entry, *index),
                cols: (from, from + cols(s, e)),
                text: full.clone(),
            })
        })
        .collect()
}

/// `[start, end)` 这一行里附件块占的列（从内容开头算）和文件：点它、悬停它照链接办（「她的回答：Markdown」第 10 条）。
fn links(text: &str, start: usize, end: usize, pieces: &[Piece]) -> Vec<(u16, u16, String)> {
    let cols = |a: usize, b: usize| u16::try_from(text[a..b].width()).unwrap_or(u16::MAX);
    pieces
        .iter()
        .filter_map(|p| {
            let (s, e) = (p.from.max(start), p.to.min(end));
            let file = p.file.clone().filter(|_| s < e)?;
            let from = cols(start, s);
            Some((from, from + cols(s, e), file))
        })
        .collect()
}

/// 字里每一块占的字节范围：照先后一块一块往后找它的样子（两块写出来一样也各对各）。
fn block_ranges(text: &str, chips: &[Chip]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    for c in chips {
        let Some(at) = text[from..].find(c.label.as_str()) else {
            break;
        };
        out.push((from + at, from + at + c.label.len()));
        from += at + c.label.len();
    }
    out
}

/// `[start, end)` 这一截排成几段：落在块里的（折行折到一半的也算）用块的样子，别的原色。
fn spans(
    text: &str,
    start: usize,
    end: usize,
    pieces: &[(usize, usize, Style)],
) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let mut at = start;
    for &(s, e, style) in pieces {
        let (s, e) = (s.max(start), e.min(end));
        if s >= e {
            continue;
        }
        if at < s {
            out.push(Span::styled(text[at..s].to_string(), Style::new()));
        }
        out.push(Span::styled(text[s..e].to_string(), style));
        at = e;
    }
    if at < end {
        out.push(Span::styled(text[at..end].to_string(), Style::new()));
    }
    out
}
