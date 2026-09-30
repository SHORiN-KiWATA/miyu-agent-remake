//! 你说的话（蓝图 `tui.md`「正文」第 2 条）：行首竖线，上下各多一行只有竖线的空行，竖线的颜色是发出去那一刻的权限级别。
//! 里面有粘贴块的，块照输入框里的样子写（品红字、暗紫底），整条能点：点开原地把每一块换成全文，再点换回块；
//! 悬停时块亮一档，展开着的粘的那几段铺上块的底色。附件（`[图片 1]`）也照块写，点了不展开。

use ratatui::style::Style;
use ratatui::text::Span;

use super::rows::{Ctx, Row, Target};
use crate::input::wrap;
use crate::theme;
use crate::transcript::{Chip, Entry};

/// 排成的行。`i` 是它在正文里是第几条。
pub fn rows(i: usize, entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let bar = Span::styled(
        ctx.config.layout.user_bar.clone(),
        theme::user_bar(entry.level.unwrap_or(ctx.level)),
    );
    let said = entry.text.trim_matches('\n');
    // 有粘贴块的才能点；只有附件的点了没反应（「输入框」第 12 条）。
    let clickable = entry.pasted.iter().any(|c| !c.attachment());
    let hovered = clickable && ctx.hover == Some(Target::Entry(i));
    let (text, pieces) = shaped(said, &entry.pasted, entry.open, hovered);
    let mut out = vec![ctx.row(bar.clone(), Vec::new())];
    let mut prev_end = None;
    for line in wrap(&text, ctx.width.max(1)) {
        let mut row = ctx.row(bar.clone(), spans(&text, line.start, line.end, &pieces));
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

/// 排成的字，和每一块的字节范围、样子。收着的块写块上的字，悬停时亮一档；点开了粘贴块原地换成全文，悬停时粘的
/// 那几段铺上块的底色；附件怎么都是块（「输入框」第 12 条：点了不展开、底色不掉）。
fn shaped(
    text: &str,
    chips: &[Chip],
    open: bool,
    hovered: bool,
) -> (String, Vec<(usize, usize, Style)>) {
    let chip = if hovered {
        theme::chip_hover()
    } else {
        theme::chip()
    };
    let mut out = String::with_capacity(text.len());
    let mut pieces = Vec::new();
    let mut at = 0;
    for ((start, end), c) in block_ranges(text, chips).into_iter().zip(chips) {
        out.push_str(&text[at..start]);
        let from = out.len();
        if open && !c.attachment() {
            out.push_str(c.full.trim_matches('\n'));
            if hovered {
                pieces.push((from, out.len(), theme::chip_ground()));
            }
        } else {
            out.push_str(&text[start..end]);
            pieces.push((from, out.len(), chip));
        }
        at = end;
    }
    out.push_str(&text[at..]);
    (out, pieces)
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
