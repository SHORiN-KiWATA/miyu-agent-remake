//! 输入框上面三样共用的框（蓝图 `tui.md`「斜杠命令列表」第 3 条）。

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::{Chrome, Row, draw, fit, height, inside, item, room};
use crate::theme;

fn text(rows: &[Row]) -> Vec<String> {
    rows.iter().map(|(_, l)| l.to_string()).collect()
}

#[test]
fn a_short_room_shows_fewer_items_around_the_picked_one() {
    let rows: Vec<Row> = (0..6)
        .map(|i| (Some(i), Line::raw(format!("第 {i} 条"))))
        .collect();
    assert_eq!(fit(rows.clone(), Some(2), 10).len(), 6, "放得下：原样");
    assert_eq!(
        text(&fit(rows.clone(), Some(2), 3)),
        ["第 1 条", "第 2 条", "第 3 条"],
        "从离选中那条最远的起少露"
    );
    assert!(fit(rows, Some(2), 0).is_empty());
}

#[test]
fn the_frame_takes_two_rows_when_there_is_room_for_it() {
    assert_eq!((room(7), room(3), room(2)), (5, 1, 2), "不到 3 行不画框");
    assert_eq!((height(5, 20), height(2, 2), height(0, 20)), (7, 2, 0));
    let outer = Rect::new(10, 4, 60, 7);
    assert_eq!(
        inside(outer, 14, 52),
        Rect::new(14, 5, 52, 5),
        "上下各让一行"
    );
    assert_eq!(
        inside(Rect::new(10, 4, 60, 2), 14, 52),
        Rect::new(14, 4, 52, 2)
    );
}

#[test]
fn a_rounded_frame_with_the_title_and_hint_on_its_edges_and_a_blue_picked_bar() {
    // 2026-09-30 项目主人看过对比页定的 A，再照 Cline 给选中的铺强调蓝，上面的字换深色。
    let _theme = theme::hold();
    let mut terminal = Terminal::new(TestBackend::new(30, 5)).unwrap();
    let outer = Rect::new(0, 0, 30, 5);
    let text_area = inside(outer, 2, 26);
    let lines = vec![
        item(false, vec![Span::styled("/undo", theme::dim())], None, 26),
        item(true, vec![Span::styled("/restore", theme::dim())], None, 26),
        item(
            false,
            vec![Span::styled("/compact", theme::dim())],
            None,
            26,
        ),
    ];
    let chrome = Chrome::new("命令", vec![Span::styled(" 3 条", theme::dim())]).hint("Esc 关闭");
    terminal
        .draw(|f| draw(f, outer, text_area, chrome, lines))
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    // 一行的字：宽字占两格，后面那一格跳过。
    let row = |y: u16| -> String {
        let mut out = String::new();
        let mut x = 0;
        while x < 30 {
            let symbol = buf[(x, y)].symbol();
            out.push_str(symbol);
            x += u16::try_from(symbol.width().max(1)).unwrap_or(1);
        }
        out
    };
    assert!(row(0).starts_with("╭─ 命令 3 条 ─"), "{}", row(0));
    assert!(row(0).ends_with('╮'));
    assert!(
        row(4).starts_with("╰─ Esc 关闭 ─") && row(4).ends_with('╯'),
        "{}",
        row(4)
    );
    assert!(
        row(1).starts_with("│   /undo") && row(1).ends_with('│'),
        "{}",
        row(1)
    );
    assert_eq!(
        buf[(0, 0)].fg,
        theme::dim().fg.unwrap(),
        "框线和输入框的边一个颜色"
    );
    assert_eq!(buf[(3, 0)].symbol(), "命");
    assert_eq!(
        buf[(3, 0)].fg,
        theme::dim().fg.unwrap(),
        "边框上的字和框线一个颜色"
    );
    let esc = (0..30).find(|&x| buf[(x, 4)].symbol() == "E").unwrap();
    assert_eq!(buf[(esc, 4)].fg, theme::dim().fg.unwrap(), "按键提示也是");
    // 选中的那一条：❯、名字和后面补的空格都铺强调色的底，字是深色。
    let bar = theme::picked_bar();
    assert!(row(2).contains("❯ /restore"), "{}", row(2));
    for x in 2..28 {
        let cell = &buf[(x, 2)];
        assert_eq!(
            (cell.bg, cell.fg),
            (bar.bg.unwrap(), bar.fg.unwrap()),
            "第 {x} 格"
        );
    }
    assert_eq!(
        buf[(2, 1)].bg,
        ratatui::style::Color::Reset,
        "没选中的不铺底"
    );
}

#[test]
fn without_room_for_the_frame_only_the_items_are_drawn() {
    let _theme = theme::hold();
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    let outer = Rect::new(0, 0, 20, 2);
    let lines = vec![
        item(true, vec![Span::raw("/undo")], None, 20),
        item(false, vec![Span::raw("/restore")], None, 20),
    ];
    terminal
        .draw(|f| {
            draw(
                f,
                outer,
                inside(outer, 0, 20),
                Chrome::new("命令", Vec::new()),
                lines,
            )
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    assert_eq!(buf[(0, 0)].symbol(), "❯");
    assert_eq!(buf[(2, 1)].symbol(), "/");
}
