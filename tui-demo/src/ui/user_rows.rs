//! 你说的话（蓝图 `tui.md`「正文」第 2 条）：行首竖线，上下各多一行只有竖线的空行，竖线的颜色是发出去那一刻的权限级别。
//! 里面有粘贴块的，块照输入框里的样子写（品红字、暗紫底），整条能点：点开原地把每一块换成全文，再点换回块；
//! 悬停时块亮一档，展开着的粘的那几段铺上块的底色。

use ratatui::style::Style;
use ratatui::text::Span;

use super::rows::{Ctx, Row, Target};
use crate::input::wrap;
use crate::theme;
use crate::transcript::Entry;

/// 排成的行。`i` 是它在正文里是第几条。
pub fn rows(i: usize, entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let bar = Span::styled(
        ctx.config.layout.user_bar.clone(),
        theme::user_bar(entry.level.unwrap_or(ctx.level)),
    );
    let said = entry.text.trim_matches('\n');
    let hovered = ctx.hover == Some(Target::Entry(i));
    // 点开了：块原地换成全文，照平常的字排，悬停时粘的那几段铺底色；没点开：块画成小块，悬停时亮一档。
    let (text, blocks, style) = if entry.open {
        let (text, pasted) = replaced(said, &entry.pasted);
        let shown = if hovered { pasted } else { Vec::new() };
        (text, shown, theme::chip_ground())
    } else {
        let style = if hovered {
            theme::chip_hover()
        } else {
            theme::chip()
        };
        (said.to_string(), block_ranges(said, &entry.pasted), style)
    };
    let mut out = vec![ctx.row(bar.clone(), Vec::new())];
    let mut prev_end = None;
    for line in wrap(&text, ctx.width.max(1)) {
        let mut row = ctx.row(
            bar.clone(),
            spans(&text, line.start, line.end, &blocks, style),
        );
        row.joined = prev_end == Some(line.start);
        prev_end = Some(line.end);
        out.push(row);
    }
    out.push(ctx.row(bar, Vec::new()));
    if !entry.pasted.is_empty() {
        for row in &mut out {
            row.target = Some(Target::Entry(i));
        }
    }
    out
}

/// 每一块换成全文，交回换好的字和换进去的那几段的字节范围。
fn replaced(text: &str, pasted: &[(String, String)]) -> (String, Vec<(usize, usize)>) {
    let mut out = String::with_capacity(text.len());
    let mut ranges = Vec::new();
    let mut at = 0;
    for ((start, end), (_, full)) in block_ranges(text, pasted).into_iter().zip(pasted) {
        out.push_str(&text[at..start]);
        let from = out.len();
        out.push_str(full.trim_matches('\n'));
        ranges.push((from, out.len()));
        at = end;
    }
    out.push_str(&text[at..]);
    (out, ranges)
}

/// 字里每一块粘贴占的字节范围：照先后一块一块往后找它的样子（两块写出来一样也各对各）。
fn block_ranges(text: &str, pasted: &[(String, String)]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    for (label, _) in pasted {
        let Some(at) = text[from..].find(label.as_str()) else {
            break;
        };
        out.push((from + at, from + at + label.len()));
        from += at + label.len();
    }
    out
}

/// `[start, end)` 这一截排成几段：`blocks` 里的（折行折到一半的也算）用 `style`，别的原色。
fn spans(
    text: &str,
    start: usize,
    end: usize,
    blocks: &[(usize, usize)],
    style: Style,
) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let mut at = start;
    for &(s, e) in blocks {
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
