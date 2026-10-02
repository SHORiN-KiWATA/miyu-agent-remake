//! 链接卡片排成的行（蓝图 `tui.md`「链接卡片」第 3、5 条，2026-10-02 项目主人定样子）：左边一张封面小图（`link_cover_rows`
//! 行高、最宽占三分之一），右边三行字：网站图标（`link_icon_cols` 列宽一行高）接标题（加粗），网站名，简介一行。
//! 没有封面图、图还没好的，字从左边起；终端不认图的只有字。点哪一格都是点这个链接。

use std::path::Path;

use ratatui::style::Modifier;
use ratatui::text::Span;

use super::rows::{Ctx, FigureCell, Row, clip};
use crate::core::Card;
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
        });
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
    let title_lead = icon.map_or(0, |(_, cols, _)| cols + 1);
    let title = clip(&card.title, room.saturating_sub(title_lead));
    // 空的那一行（没有网站名、没有简介）不占位（2026-10-02 项目主人报：两行的也排成了三行）。
    let lines: Vec<(u16, Span<'static>)> = [
        (
            title_lead,
            Span::styled(title, theme::md_link().add_modifier(Modifier::BOLD)),
        ),
        (0, Span::styled(clip(&card.site, room), theme::dim())),
        (0, Span::styled(clip(&card.description, room), theme::dim())),
    ]
    .into_iter()
    .filter(|(_, text)| !text.content.trim().is_empty())
    .collect();
    let cover_rows = cover.map_or(0, |(_, _, rows)| usize::from(rows));
    let cover_cols = cover.map_or(0, |(_, cols, _)| cols);
    let height = cover_rows.max(lines.len());
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
            let mut row = ctx.led_row(slot.clone(), lead.to_vec(), content);
            row.copy = i < lines.len();
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
mod tests {
    use crate::core::Card;
    use crate::transcript::{Kind, Transcript};
    use crate::ui::test_support::Fixture;

    fn text(rows: &[crate::ui::rows::Row]) -> Vec<String> {
        rows.iter()
            .map(|r| r.line.to_string().trim().to_string())
            .collect()
    }

    #[test]
    fn a_link_alone_on_its_line_becomes_a_card_and_an_inline_one_stays() {
        // 2026-10-02 项目主人定：只给独占一行的链接做，就地换成卡片；回答写完了才换。
        let f = Fixture::new();
        f.cards.borrow_mut().got_card(
            "https://a.dev/x".into(),
            Some(Card {
                title: "A 的文章".into(),
                description: "讲点什么".into(),
                site: "a.dev".into(),
                image: None,
                icon: None,
            }),
        );
        let mut t = Transcript::default();
        t.note(
            Kind::Reply,
            "看这个：\nhttps://a.dev/x\n还有 https://a.dev/x 也行".into(),
        );
        let rows = crate::ui::rows::entry_rows(0, &t.entries[0], &f.ctx());
        let shown = text(&rows);
        assert!(shown.iter().any(|l| l == "A 的文章"), "{shown:#?}");
        assert!(shown.iter().any(|l| l == "a.dev"), "{shown:#?}");
        assert!(shown.iter().any(|l| l == "讲点什么"), "{shown:#?}");
        assert!(
            !shown.iter().any(|l| l == "https://a.dev/x"),
            "那一行换掉了：{shown:#?}"
        );
        assert!(
            shown
                .iter()
                .any(|l| l.contains("还有 https://a.dev/x 也行")),
            "夹在字里的不动：{shown:#?}"
        );
        let card = rows
            .iter()
            .find(|r| r.line.to_string().contains("A 的文章"))
            .unwrap();
        assert!(
            card.links
                .iter()
                .any(|(_, _, url)| url == "https://a.dev/x"),
            "点卡片是点这个链接"
        );
        // 她还在写这一条：不换。
        let mut ctx = f.ctx();
        ctx.writing = Some(t.entries[0].id);
        let shown = text(&crate::ui::rows::entry_rows(0, &t.entries[0], &ctx));
        assert!(shown.iter().any(|l| l == "https://a.dev/x"), "{shown:#?}");
    }

    #[test]
    fn a_card_has_a_blank_line_around_it_skips_empty_lines_and_links_only_its_text() {
        // 2026-10-02 项目主人报：卡片上下没有空行；没有简介的两行也排成三行；悬停时整行（连空的地方）画下划线。
        let f = Fixture::new();
        f.cards.borrow_mut().got_card(
            "https://w.org/rust".into(),
            Some(Card {
                title: "Rust".into(),
                description: String::new(),
                site: "w.org".into(),
                image: None,
                icon: None,
            }),
        );
        let mut t = Transcript::default();
        t.note(Kind::Reply, "介绍：\nhttps://w.org/rust\n就这些。".into());
        let rows = crate::ui::rows::entry_rows(0, &t.entries[0], &f.ctx());
        let shown = text(&rows);
        assert_eq!(
            shown,
            ["介绍：", "", "Rust", "w.org", "", "就这些。"],
            "上下各空一行、没有简介的只两行"
        );
        let title = &rows[2];
        assert_eq!(title.links.len(), 1);
        let (from, to, _) = &title.links[0];
        assert_eq!((*from, *to), (0, 4), "只有字那一截能点：{:?}", title.links);
    }

    #[test]
    fn a_bare_address_alone_in_what_you_said_becomes_a_card_too() {
        let f = Fixture::new();
        f.cards.borrow_mut().got_card(
            "https://a.dev/x".into(),
            Some(Card {
                title: "A 的文章".into(),
                description: String::new(),
                site: "a.dev".into(),
                image: None,
                icon: None,
            }),
        );
        let mut t = Transcript::default();
        t.user("帮我看看\nhttps://a.dev/x".into(), Vec::new());
        let rows = crate::ui::rows::entry_rows(0, &t.entries[0], &f.ctx());
        let shown = text(&rows);
        assert!(shown.iter().any(|l| l.ends_with("A 的文章")), "{shown:#?}");
        assert!(
            !shown.iter().any(|l| l.ends_with("https://a.dev/x")),
            "{shown:#?}"
        );
        let card = rows
            .iter()
            .find(|r| r.line.to_string().contains("A 的文章"))
            .unwrap();
        assert!(
            card.line.to_string().contains('┃'),
            "竖线照样在：{}",
            card.line
        );
    }

    #[test]
    fn a_link_without_a_card_is_asked_for_once_and_stays_a_link() {
        let f = Fixture::new();
        let mut t = Transcript::default();
        t.note(Kind::Reply, "https://b.dev".into());
        let shown = text(&crate::ui::rows::entry_rows(0, &t.entries[0], &f.ctx()));
        assert!(shown.iter().any(|l| l == "https://b.dev"), "{shown:#?}");
        let (cards, _) = f.cards.borrow_mut().take();
        assert_eq!(cards, ["https://b.dev"], "记进单子");
    }
}
