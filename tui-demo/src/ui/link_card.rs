//! 链接卡片排成的行（蓝图 `tui.md`「链接卡片」第 3、5 条，2026-10-02 项目主人定样子）：左边一张封面小图（`link_cover_rows`
//! 行高、最宽占三分之一），右边三行字：网站图标（`link_icon_cols` 列宽一行高）接标题（加粗），站名与作者，简介一行；视频有封面时在卡片下方另排播放标记与时长。
//! 没有封面图、图还没好的，字从左边起；终端不认图的只有字。点哪一格都是点这个链接。

use std::path::Path;

use ratatui::style::Modifier;
use ratatui::text::Span;

use super::rows::{Ctx, FigureCell, Row, clip};
use crate::core::{Card, CardKind};
use crate::figures::Look;
use crate::markdown::FigureKind;
use crate::theme;

/// 一张卡片的几行；`slot` 是两格槽（你说的话是竖线），`lead` 是这一行前面的引子（列表缩进、引用的竖线）。
pub fn rows(
    slot: &Span<'static>,
    lead: &[Span<'static>],
    url: &str,
    card: &Card,
    ctx: &Ctx,
) -> Vec<Row> {
    let layout = &ctx.config.layout;
    let mut pending = false;
    // 封面图：最多 `link_cover_rows` 行高、三分之一宽。
    let cover = card
        .image
        .as_deref()
        .and_then(|blob| ctx.cards.borrow_mut().file(blob).map(Path::to_path_buf))
        .and_then(|path| {
            let cols = (ctx.width / 3).max(1);
            let look = ctx.figures.borrow_mut().look(
                FigureKind::Image,
                &path.display().to_string(),
                (None, None),
                cols,
                layout.link_cover_rows,
            );
            ready(look, ctx, &mut pending)
        })
        .filter(|(_, cols, _)| cols.saturating_add(2) < ctx.width);
    let icon = card
        .icon
        .as_deref()
        .and_then(|blob| ctx.cards.borrow_mut().file(blob).map(Path::to_path_buf))
        .and_then(|path| {
            let look = ctx.figures.borrow_mut().look(
                FigureKind::Image,
                &path.display().to_string(),
                (None, None),
                layout.link_icon_cols,
                1,
            );
            ready(look, ctx, &mut pending)
        });
    // 字从封面图右边空两格起。
    let left = cover.map_or(0, |(_, cols, _)| cols + 2);
    let room = ctx.width.saturating_sub(left).max(1);
    let icon = icon.filter(|(_, cols, _)| *cols <= room);
    let title_lead = icon.map_or(0, |(_, cols, _)| cols.saturating_add(1).min(room));
    let title = clip(&card.title, room.saturating_sub(title_lead));
    // 空的那一行（没有网站名、没有简介）不占位（2026-10-02 项目主人报：两行的也排成了三行）。
    let site = [
        card.site.trim(),
        card.author.as_deref().unwrap_or_default().trim(),
    ]
    .into_iter()
    .filter(|text| !text.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let lines: Vec<(u16, Span<'static>)> = [
        (
            title_lead,
            Span::styled(title, theme::md_link().add_modifier(Modifier::BOLD)),
        ),
        (0, Span::styled(clip(&site, room), theme::dim())),
        (0, Span::styled(clip(&card.description, room), theme::dim())),
    ]
    .into_iter()
    .filter(|(_, text)| !text.content.trim().is_empty())
    .collect();
    let cover_rows = cover.map_or(0, |(_, _, rows)| usize::from(rows));
    let cover_cols = cover.map_or(0, |(_, cols, _)| cols);
    let body_height = cover_rows.max(lines.len());
    let footer = (card.kind == CardKind::Video && cover.is_some()).then(|| {
        let mark = &ctx.config.icons.video;
        let text = match card.duration.filter(|seconds| *seconds > 0) {
            Some(seconds) => format!("{mark} {}", duration(seconds)),
            None => mark.clone(),
        };
        clip(&text, ctx.width)
    });
    let height = body_height + usize::from(footer.is_some());
    (0..height)
        .map(|i| {
            let mut content = Vec::new();
            // 能点的只是封面图那一截和字那一截：悬停时整行加下划线，空的地方也画（同一天项目主人报）。
            let mut links = Vec::new();
            if i < cover_rows {
                links.push((0, cover_cols, url.to_string()));
            }
            if let Some((pad, text)) = lines.get(i) {
                let start = left + pad;
                content.push(Span::raw(" ".repeat(usize::from(start))));
                let end = start + u16::try_from(text.width()).unwrap_or(u16::MAX);
                content.push(text.clone());
                links.push((start, end, url.to_string()));
            }
            if i == body_height
                && let Some(text) = &footer
            {
                let end = u16::try_from(Span::raw(text.as_str()).width()).unwrap_or(u16::MAX);
                content.push(Span::styled(text.clone(), theme::dim()));
                if end > 0 {
                    links.push((0, end, url.to_string()));
                }
            }
            let mut row = ctx.led_row(slot.clone(), lead.to_vec(), content);
            row.copy = i < lines.len() || (i == body_height && footer.is_some());
            row.links = links;
            row.figure_pending = pending;
            if let Some((key, _, _)) = cover.filter(|(_, _, rows)| (i as u16) < *rows) {
                row.figure = Some(FigureCell {
                    key,
                    row: u16::try_from(i).unwrap_or(0),
                    x: 0,
                });
            }
            if let Some((key, _, _)) = icon.filter(|_| i == 0) {
                row.icon = Some(FigureCell {
                    key,
                    row: 0,
                    x: left,
                });
            }
            row
        })
        .collect()
}

/// 视频秒数写成 m:ss 或 h:mm:ss；分钟、小时不补零。
fn duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes >= 60 {
        format!("{}:{:02}:{seconds:02}", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// 这一行是空的（卡片前后补空行时认它，不补出两行空的）。
pub fn blank(row: &Row) -> bool {
    row.plain.trim().is_empty() && row.figure.is_none()
}

/// 做好了的交回键、几列、几行；在做的记一下（做好了要重排），终端不认的、做坏的是 `None`。
fn ready(look: Look, ctx: &Ctx, pending: &mut bool) -> Option<(u64, u16, u16)> {
    match look {
        Look::Ready { key, rows } => {
            let cols = ctx.figures.borrow().cols(key)?;
            Some((key, cols, rows))
        }
        Look::Pending => {
            *pending = true;
            None
        }
        Look::Unsupported | Look::Failed => None,
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod site_tests;
