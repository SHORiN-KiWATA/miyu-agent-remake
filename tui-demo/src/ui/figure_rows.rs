//! 正文里的图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 2、6 条）：排行时一张图占几行就是几行，
//! 没好是一行占位，画不成的写源码；画的时候照露出来的那一截切片画。

use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::Widget;
use ratatui_image::sliced::{SignedPosition, SlicedImage};

use super::row_cache::Rows;
use super::rows::{Ctx, FigureCell, Row, md_row};
use crate::figures::{self, Figures, Look};
use crate::markdown::Figure;
use crate::theme;

/// 一张图排成的行。`lead` 是这一行前面的引子（列表缩进、引用的竖线），图接在它后面。
pub fn rows(lead: Vec<Span<'static>>, figure: &Figure, ctx: &Ctx) -> Vec<Row> {
    let lead_width: usize = lead.iter().map(Span::width).sum();
    let text_cols = ctx
        .width
        .saturating_sub(u16::try_from(lead_width).unwrap_or(u16::MAX))
        .max(1);
    // 最多多大照这一种图的比例（图片最多三分之一屏、正文宽的六成，蓝图第 3 条）。
    let (cols, rows) =
        figures::room(&ctx.config.figures, figure.kind).fit(text_cols, ctx.screen_rows);
    let look = ctx.figures.borrow_mut().look(
        figure.kind,
        &figure.source,
        (figure.width, figure.height),
        cols,
        rows,
    );
    match look {
        Look::Unsupported | Look::Failed => figure
            .fallback
            .iter()
            .map(|line| md_row(line.clone(), ctx))
            .collect(),
        Look::Pending => {
            let text = Span::styled(ctx.config.text.figure_pending.clone(), theme::dim());
            let mut row = ctx.led_row(ctx.blank_slot(), lead, vec![text]);
            row.copy = false;
            vec![row]
        }
        Look::Ready { key, rows } => {
            let mut out: Vec<Row> = (0..rows)
                .map(|i| {
                    let mut row = ctx.led_row(ctx.blank_slot(), lead.clone(), Vec::new());
                    row.copy = false;
                    row.figure = Some(FigureCell { key, row: i });
                    row
                })
                .collect();
            let zoom = ctx.figures.borrow().get(key).and_then(|d| d.zoom.clone());
            if let Some(file) = zoom {
                out.push(zoom_row(lead, &file, ctx));
            }
            out
        }
    }
}

/// 图下面那一行暗色的「点开看大图」，整行是一个链接，复制时不带（蓝图第 4 条）。
fn zoom_row(lead: Vec<Span<'static>>, file: &std::path::Path, ctx: &Ctx) -> Row {
    let text = ctx.config.text.figure_zoom.clone();
    let width = u16::try_from(Span::raw(text.as_str()).width()).unwrap_or(u16::MAX);
    let mut row = ctx.led_row(
        ctx.blank_slot(),
        lead,
        vec![Span::styled(text, theme::dim())],
    );
    row.links = vec![(0, width, file.display().to_string())];
    row.copy = false;
    row
}

/// 画视口里露出来的图：每张图从它第 0 行该在的位置画起（在视口上面的是负的），
/// 视口外的那一截由 `SlicedImage` 切掉。`first` 是视口第一行是正文的第几行。露出来的记下露过：编好的图记满了
/// 扔没露出来的，被扔了的这一帧先空着、交给后台重做（蓝图「图片、公式和 mermaid 图」第 6 条）。
pub fn draw(buf: &mut Buffer, area: Rect, rows: &Rows, first: usize, figures: &mut Figures) {
    figures.next_frame();
    let mut drawn = HashSet::new();
    let visible = rows.window(first, usize::from(area.height));
    for (i, row) in visible {
        let Some(FigureCell { key, row: at }) = row.figure else {
            continue;
        };
        if !drawn.insert(key) {
            continue;
        }
        let Some(figure) = figures.shown(key) else {
            continue;
        };
        let top = i64::try_from(i - first).unwrap_or(0) - i64::from(at);
        let position = SignedPosition {
            x: i16::try_from(row.content_x).unwrap_or(i16::MAX),
            y: i16::try_from(top).unwrap_or(i16::MIN),
        };
        SlicedImage::new(&figure.protocol, position).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::mpsc;
    use std::time::Duration;

    use miyu_store::human::Human;
    use ratatui_image::picker::{Picker, ProtocolType};

    use super::rows;
    use crate::config::Config;
    use crate::core::Level;
    use crate::figures::{self, Figures, Graphics};
    use crate::markdown::{self, Figure, FigureKind};
    use crate::ui::rows::{Ctx, MdCache};

    fn figure(config: &Config, text: &str) -> Figure {
        let kit = markdown::Kit {
            languages: &config.languages,
            math: &config.math,
            labels: &config.text.markdown,
        };
        markdown::render(text, 60, &kit, &[])
            .into_iter()
            .find_map(|l| l.figure)
            .unwrap()
    }

    fn plain(rows: &[crate::ui::rows::Row]) -> Vec<String> {
        rows.iter().map(|r| r.plain.clone()).collect()
    }

    #[test]
    fn a_figure_is_a_placeholder_then_rows_of_image_and_source_without_images() {
        let config = Config::builtin().unwrap();
        let human = Human::default();
        let md = RefCell::new(MdCache::new());
        let ctx = |figures| Ctx {
            config: &config,
            human: &human,
            indent: String::new(),
            width: 60,
            hover: None,
            frame: 0,
            md: &md,
            figures,
            level: Level::Workspace,
            screen_rows: 40,
        };
        let math = figure(&config, "$$\\frac{a+1}{b}$$");
        // 终端显示不了图：写一行 Unicode。
        let plain_terminal = RefCell::new(Figures::start(None, &config.figures, None, |_| true));
        assert_eq!(
            plain(&rows(Vec::new(), &math, &ctx(&plain_terminal))),
            ["(a+1)/b"]
        );
        // 能显示图：先是一行占位，后台做好了是几行图，不复制。
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Kitty);
        let (sender, done) = mpsc::channel();
        let kitty = RefCell::new(Figures::start(
            Some(Graphics { picker }),
            &config.figures,
            None,
            move |d| sender.send(d).is_ok(),
        ));
        let first = rows(Vec::new(), &math, &ctx(&kitty));
        assert_eq!(
            plain(&first),
            std::slice::from_ref(&config.text.figure_pending)
        );
        // 并行的别的测试可能换了主题（全局的「换过几次」变了，键跟着变），那就再做一张：等到做好为止。
        let mut drawn = first;
        for _ in 0..5 {
            let finished = done.recv_timeout(Duration::from_secs(20)).unwrap();
            kitty.borrow_mut().done(finished);
            drawn = rows(Vec::new(), &math, &ctx(&kitty));
            if drawn.iter().all(|r| r.figure.is_some()) {
                break;
            }
        }
        assert!(drawn.len() >= 2, "分式至少两行高");
        assert!(drawn.iter().all(|r| r.figure.is_some() && !r.copy));
        let at: Vec<u16> = drawn
            .iter()
            .filter_map(|r| r.figure)
            .map(|f| f.row)
            .collect();
        assert_eq!(
            at,
            (0..u16::try_from(drawn.len()).unwrap()).collect::<Vec<_>>()
        );
    }

    /// 等后台做好这一张，交回占几行。
    fn drawn_rows(
        figure: &Figure,
        ctx: &Ctx,
        done: &mpsc::Receiver<crate::figures::Done>,
    ) -> usize {
        for _ in 0..5 {
            let got = rows(Vec::new(), figure, ctx);
            if got.iter().all(|r| r.figure.is_some()) {
                return got.len();
            }
            let finished = done.recv_timeout(Duration::from_secs(20)).unwrap();
            ctx.figures.borrow_mut().done(finished);
        }
        panic!("没做好");
    }

    #[test]
    fn pictures_take_a_third_of_the_window_and_six_tenths_of_the_width() {
        // 2026-09-30 项目主人：半屏还是太大。45 行高、正文 100 列，一格 10×20 像素。
        let config = Config::builtin().unwrap();
        let human = Human::default();
        let md = RefCell::new(MdCache::new());
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Kitty);
        let (sender, done) = mpsc::channel();
        let figures = RefCell::new(Figures::start(
            Some(Graphics { picker }),
            &config.figures,
            None,
            move |d| sender.send(d).is_ok(),
        ));
        let ctx = Ctx {
            config: &config,
            human: &human,
            indent: String::new(),
            width: 100,
            hover: None,
            frame: 0,
            md: &md,
            figures: &figures,
            level: Level::Workspace,
            screen_rows: 45,
        };
        let dir = std::env::temp_dir().join(format!("miyu-room-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let picture = |name: &str, w: u32, h: u32| {
            let path = dir.join(name);
            image::RgbaImage::from_pixel(w, h, image::Rgba([0, 128, 255, 255]))
                .save(&path)
                .unwrap();
            figure(&config, &format!("![]({})", path.display()))
        };
        // 3:1 的横幅：宽最多 60 格（600 像素），高 200 像素是 10 行。
        let banner = picture("banner.png", 3000, 1000);
        assert_eq!(drawn_rows(&banner, &ctx, &done), 10, "宽最多正文宽的六成");
        // 16:9 的壁纸：60 格宽是 17 行，高最多三分之一屏 15 行。
        let wallpaper = picture("wallpaper.png", 1600, 900);
        assert_eq!(drawn_rows(&wallpaper, &ctx, &done), 15, "高最多三分之一屏");
        std::fs::remove_dir_all(&dir).unwrap_or_default();
        // mermaid、公式、`<svg>` 里是字：照旧最多正文宽、半屏。
        for kind in [FigureKind::Mermaid, FigureKind::Math, FigureKind::Svg] {
            assert_eq!(figures::room(&config.figures, kind).fit(100, 45), (100, 22));
        }
    }
}
