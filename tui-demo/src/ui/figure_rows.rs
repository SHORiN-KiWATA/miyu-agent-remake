//! 正文里的图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 2、6 条）：排行时一张图占几行就是几行，
//! 没好是一行占位，画不成的写源码；画的时候照露出来的那一截切片画。

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::Widget;
use ratatui_image::sliced::{SignedPosition, SlicedImage};

use super::row_cache::Rows;
use super::rows::{Ctx, FigureCell, Row, md_row};
use crate::diagrams::{self, Asked};
use crate::figures::{self, Figures, Look};
use crate::markdown::{Figure, FigureKind};
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
    let look = match figure.kind {
        FigureKind::Mermaid => diagram(&figure.source, ctx, cols, rows),
        kind => ctx.figures.borrow_mut().look(
            kind,
            &figure.source,
            (figure.width, figure.height),
            cols,
            rows,
        ),
    };
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
            row.figure_pending = true;
            vec![row]
        }
        Look::Ready { key, rows } => {
            let mut out: Vec<Row> = (0..rows)
                .map(|i| {
                    let mut row = ctx.led_row(ctx.blank_slot(), lead.clone(), Vec::new());
                    row.copy = false;
                    row.figure = Some(FigureCell { key, row: i, x: 0 });
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

/// mermaid 图：SVG 由核心画（蓝图第 4 条），没回来是在做，核心拒了的写源码；回来了换成主题色交给后台栅格化。
/// 终端显示不了图的不问核心。
fn diagram(source: &str, ctx: &Ctx, cols: u16, rows: u16) -> Look {
    if !ctx.figures.borrow().shows() {
        return Look::Unsupported;
    }
    let svg = match ctx.diagrams.borrow_mut().diagram(source) {
        Asked::Waiting => return Look::Pending,
        Asked::Refused => return Look::Failed,
        Asked::Got(rendered) => diagrams::paint(rendered, theme::diagram_rgb()),
    };
    ctx.figures
        .borrow_mut()
        .look(FigureKind::Mermaid, &svg, (None, None), cols, rows)
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

/// 这一格是图占着的：kitty 的占位符，或者别的协议照原样留着不比对的格子。
pub fn is_picture(cell: &ratatui::buffer::Cell) -> bool {
    cell.diff_option == ratatui::buffer::CellDiffOption::Skip
        || cell.symbol().starts_with('\u{10EEEE}')
}

/// 一张图画在哪：做好的图的键、第 0 行在视口第几行（上面的是负的）、从第几列画起。
type Placed = (u64, i64, u16);

/// 画视口里露出来的图：每张图从它第 0 行该在的位置画起（在视口上面的是负的），只画进它自己那几行（露出来的第一行到
/// 最后一行），别的切掉——不然图的头被切掉时（进行中那一段只露最新的十几行，2026-10-10 项目主人截图）切掉的那一截会盖到
/// 前面的字上。`first` 是视口第一行是正文的第几行。露出来的记下露过：编好的图记满了扔没露出来的，被扔了的这一帧先空着、
/// 交给后台重做（蓝图「图片、公式和 mermaid 图」第 6 条）。
pub fn draw(buf: &mut Buffer, area: Rect, rows: &Rows, first: usize, figures: &mut Figures) {
    figures.next_frame();
    // 每张图（照它第 0 行在哪认：同一张图出现在两处的各画各的）露在视口里的第一行、最后一行。
    let mut spans: Vec<(Placed, (u16, u16))> = Vec::new();
    let visible = rows.window(first, usize::from(area.height));
    for (i, row) in visible {
        let at_row = u16::try_from(i - first).unwrap_or(u16::MAX);
        for FigureCell { key, row: at, x } in row.figure.into_iter().chain(row.icon) {
            let top = i64::from(at_row) - i64::from(at);
            let id = (key, top, row.content_x + x);
            match spans.iter_mut().find(|(k, _)| *k == id) {
                Some((_, (_, last))) => *last = at_row,
                None => spans.push((id, (at_row, at_row))),
            }
        }
    }
    for ((key, top, x), (from, to)) in spans {
        let Some(figure) = figures.shown(key) else {
            continue;
        };
        let own = Rect::new(area.x, area.y + from, area.width, to - from + 1);
        let position = SignedPosition {
            x: i16::try_from(x).unwrap_or(i16::MAX),
            y: i16::try_from(top - i64::from(from)).unwrap_or(i16::MIN),
        };
        SlicedImage::new(&figure.protocol, position).render(own, buf);
    }
}

#[cfg(test)]
mod picture_cells {
    use ratatui::buffer::{Cell, CellDiffOption};

    use super::is_picture;

    #[test]
    fn cells_held_by_a_picture_are_known() {
        // 2026-10-02 项目主人报：悬停链接卡片时下划线叠在封面图上。画下划线时跳过图占着的格子。
        let mut kitty = Cell::default();
        kitty.set_symbol("\u{10EEEE}\u{0305}");
        assert!(is_picture(&kitty));
        let mut skipped = Cell::default();
        skipped.set_diff_option(CellDiffOption::Skip);
        assert!(is_picture(&skipped));
        let mut text = Cell::default();
        text.set_symbol("G");
        assert!(!is_picture(&text));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::mpsc;
    use std::time::Duration;

    use crate::human::Human;
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
        let md = RefCell::new(MdCache::new(8));
        let cards = RefCell::default();
        let book = RefCell::default();
        let ctx = |figures| Ctx {
            config: &config,
            human: &human,
            indent: String::new(),
            width: 60,
            hover: None,
            frame: 0,
            md: &md,
            figures,
            cards: &cards,
            diagrams: &book,
            writing: None,
            level: Level::Workspace,
            screen_rows: 40,
        };
        let math = figure(&config, "$$\\frac{a+1}{b}$$");
        // 终端显示不了图：写成 Unicode，分式上下摞。
        let plain_terminal = RefCell::new(Figures::start(None, &config.figures, None, |_| true));
        assert_eq!(
            plain(&rows(Vec::new(), &math, &ctx(&plain_terminal))),
            [" a+1", "─────", "  b"]
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

    #[test]
    fn a_mermaid_diagram_waits_for_the_core_then_draws_and_a_refusal_is_source() {
        let config = Config::builtin().unwrap();
        let human = Human::default();
        let md = RefCell::new(MdCache::new(8));
        let book = RefCell::new(crate::diagrams::Diagrams::default());
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
            width: 60,
            hover: None,
            frame: 0,
            md: &md,
            figures: &figures,
            cards: &RefCell::default(),
            diagrams: &book,
            writing: None,
            level: Level::Workspace,
            screen_rows: 40,
        };
        let graph = figure(&config, "```mermaid\ngraph TD\nA-->B\n```");
        // 没回来：一行占位，单子上有这份源码，只问一次。
        let pending = std::slice::from_ref(&config.text.figure_pending);
        assert_eq!(plain(&rows(Vec::new(), &graph, &ctx)), pending);
        assert_eq!(plain(&rows(Vec::new(), &graph, &ctx)), pending);
        assert_eq!(
            book.borrow_mut().take(),
            std::slice::from_ref(&graph.source)
        );
        // 核心拒了（画不出、没编进 mermaid）：照代码块写源码。
        let bad = figure(&config, "```mermaid\n坏的\n```");
        rows(Vec::new(), &bad, &ctx);
        assert_eq!(book.borrow_mut().take(), std::slice::from_ref(&bad.source));
        book.borrow_mut().got(bad.source.clone(), None);
        assert_eq!(
            plain(&rows(Vec::new(), &bad, &ctx)),
            plain(
                &bad.fallback
                    .iter()
                    .map(|l| crate::ui::rows::md_row(l.clone(), &ctx))
                    .collect::<Vec<_>>()
            )
        );
        // 回来了：换好色交给后台，画成几行图。
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="80"><rect x="1" y="1" width="198" height="78" fill="none" stroke="#040506"/><text x="10" y="40" fill="#010203">A</text></svg>"##;
        let marks = crate::core::Marks {
            label: "#070809".to_string(),
            line: "#040506".to_string(),
            text: "#010203".to_string(),
        };
        let rendered = crate::core::Rendered {
            svg: svg.to_string(),
            marks,
        };
        book.borrow_mut().got(graph.source.clone(), Some(rendered));
        assert!(drawn_rows(&graph, &ctx, &done) >= 1);
        assert!(!book.borrow().pending(), "回来过的不再问");
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
        let md = RefCell::new(MdCache::new(8));
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
            cards: &RefCell::default(),
            diagrams: &RefCell::default(),
            writing: None,
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
        // 工具结果里的图（read 读的）：长宽各是回答里的图的一半（2026-10-10 项目主人：「有现在的四分之一就差不多了」）。
        assert_eq!(
            figures::room(&config.figures, FigureKind::Preview).fit(100, 45),
            (30, 7)
        );
        // mermaid、公式、`<svg>` 里是字：照旧最多正文宽、半屏。
        for kind in [FigureKind::Mermaid, FigureKind::Math, FigureKind::Svg] {
            assert_eq!(figures::room(&config.figures, kind).fit(100, 45), (100, 22));
        }
    }

    #[test]
    fn a_picture_with_its_top_cut_off_does_not_spill_over_the_rows_above() {
        // 2026-10-10 项目主人截图：read 读的图在进行中那一段里（只露最新的十几行，上面的不画），整张图往上盖住了前面的字。
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        use ratatui::text::Span;

        use crate::ui::row_cache::Rows;
        let config = Config::builtin().unwrap();
        let human = Human::default();
        let md = RefCell::new(MdCache::new(8));
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
            cards: &RefCell::default(),
            diagrams: &RefCell::default(),
            writing: None,
            level: Level::Workspace,
            screen_rows: 45,
        };
        let dir = std::env::temp_dir().join(format!("miyu-spill-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("banner.png");
        image::RgbaImage::from_pixel(3000, 1000, image::Rgba([0, 128, 255, 255]))
            .save(&path)
            .unwrap();
        let banner = figure(&config, &format!("![]({})", path.display()));
        assert_eq!(drawn_rows(&banner, &ctx, &done), 10);
        let picture = rows(Vec::new(), &banner, &ctx);
        std::fs::remove_dir_all(&dir).unwrap_or_default();
        // 前面两行别的字，图的头三行切掉了。
        let text = ctx.row(ctx.blank_slot(), vec![Span::raw("字")]);
        let mut list = vec![text.clone(), text];
        list.extend(picture.into_iter().skip(3));
        let cache = Rows::of_rows(list);
        let area = Rect::new(0, 0, 100, 12);
        let mut buf = Buffer::empty(area);
        super::draw(&mut buf, area, &cache, 0, &mut figures.borrow_mut());
        let held = |y: u16| (0..area.width).any(|x| super::is_picture(&buf[(x, y)]));
        assert!(!held(0) && !held(1), "图没盖到前面的字上");
        assert!(held(2) && held(8), "露出来的那一截照画");
    }
}
