//! 正文区：视口第一行露哪一行、点开的一块铺底色的范围。
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::first_row;
use crate::body_view::BodyView;
use crate::ui::rows::Row;

fn rows(n: usize) -> crate::ui::row_cache::Rows {
    let row = Row {
        line: Line::default(),
        target: None,
        shade: false,
        plain: String::new(),
        content_x: 0,
        joined: false,
        links: Vec::new(),
        copy: true,
        figure: None,
        figure_pending: false,
    };
    vec![row; n].into()
}

#[test]
fn a_shaded_block_starts_at_the_bar_and_ends_two_past_the_text() {
    // 2026-09-30 项目主人：原来铺满正文区，两边超出字太多。正文区从第 10 列起、缩进 2 格，字从第 14 列起、宽 73。
    let area = Rect::new(10, 0, 80, 20);
    assert_eq!(super::shade_span(area, 2, 73), (12, 77));
}

#[test]
fn shrinking_content_does_not_pull_the_rows_back_down() {
    let area = Rect::new(0, 0, 10, 10);
    let mut view = BodyView::default();
    // 30 行露 10 行：跟着最新的，第一行是第 20 行。
    assert_eq!(first_row(&rows(30), area, &mut view, true), 20);
    // 思考的预览收起，少了 8 行：已经顶上去的不掉下来。
    assert_eq!(first_row(&rows(22), area, &mut view, true), 20);
    // 新的字来了，接着往下走。
    assert_eq!(first_row(&rows(35), area, &mut view, true), 25);
    // 撤销藏起了几轮，只剩 27 行：已经顶上去的不掉下来。
    assert_eq!(first_row(&rows(27), area, &mut view, true), 25);
    // 接着发出一句话，多了 4 行：不往回跳，照旧只往下走。
    view.follow();
    assert_eq!(first_row(&rows(31), area, &mut view, true), 25);
    // 滚上去看，再发一句话：回到最底下。
    view.top = Some(5);
    assert_eq!(first_row(&rows(31), area, &mut view, true), 5);
    view.follow();
    assert_eq!(first_row(&rows(36), area, &mut view, true), 26);
}

#[test]
fn a_cleared_screen_keeps_its_place_when_the_window_gets_wider() {
    // 2026-10-02 项目主人报：半屏时清过屏，最大化以后正文全空，滚一下才回来。清屏那一刻记的是第几行，窗口变宽、
    // 长段落折的行少了，那个行号落到了内容后面。
    let blank = rows(1).get(0).unwrap().clone();
    let narrow = crate::ui::row_cache::Rows::of_chunks(
        &[
            (40, Some(0)),
            (1, None),
            (40, Some(1)),
            (1, None),
            (3, Some(2)),
        ],
        &blank,
    );
    let wide = crate::ui::row_cache::Rows::of_chunks(
        &[
            (10, Some(0)),
            (1, None),
            (10, Some(1)),
            (1, None),
            (3, Some(2)),
        ],
        &blank,
    );
    let mut view = BodyView::default();
    let half = Rect::new(0, 0, 40, 10);
    // 前两条以后清了屏：清屏那一行是第 81 行，第三条从第 82 行起，往下露。
    view.rows =
        crate::ui::row_cache::Rows::of_chunks(&[(40, Some(0)), (1, None), (40, Some(1))], &blank);
    view.area = half;
    view.clear();
    assert_eq!(first_row(&narrow, half, &mut view, true), 81);
    view.rows = narrow.clone();
    // 变宽：清屏那一行跟着第二条的末尾走，第三条照样露着。
    let max = Rect::new(0, 0, 80, 10);
    let first = first_row(&wide, max, &mut view, true);
    assert_eq!(first, 21, "清屏那一行跟着第二条走");
    assert!(first < wide.len(), "不空着");
}

#[test]
fn opening_the_bottom_line_pushes_up_to_show_what_opened() {
    // 2026-10-02 项目主人报：最底下的撤销那一行点开以后什么都看不见。点开的那一行留在原位、展开的往下长，
    // 长到视口外面去了：要露出展开的内容，往上推，但被点的那一行不推出视口。
    use crate::ui::rows::Target;
    let mut all = rows(30);
    let mut v: Vec<Row> = (0..30).map(|i| all.get(i).unwrap().clone()).collect();
    for row in &mut v[25..30] {
        row.target = Some(Target::Entry(5));
    }
    all = v.into();
    let area = Rect::new(0, 0, 10, 10);
    let mut view = BodyView::default();
    view.anchor = Some((Target::Entry(5), 9));
    assert_eq!(
        first_row(&all, area, &mut view, true),
        20,
        "展开的五行都露出来"
    );
    // 展开的比视口还高：被点的那一行顶到最上面为止。
    let mut tall: Vec<Row> = (0..40).map(|_| all.get(0).unwrap().clone()).collect();
    for row in &mut tall[25..40] {
        row.target = Some(Target::Entry(5));
    }
    let tall: crate::ui::row_cache::Rows = tall.into();
    let mut view = BodyView::default();
    view.anchor = Some((Target::Entry(5), 9));
    view.top = Some(0);
    assert_eq!(first_row(&tall, area, &mut view, true), 25);
}

#[test]
fn a_menu_opening_and_closing_does_not_leave_a_gap() {
    let tall = Rect::new(0, 0, 10, 10);
    let short = Rect::new(0, 0, 10, 5);
    let mut view = BodyView::default();
    assert_eq!(first_row(&rows(30), tall, &mut view, true), 20);
    // 命令列表打开，视口矮了 5 行：贴着底边，往上让。
    assert_eq!(first_row(&rows(30), short, &mut view, true), 25);
    // 列表关掉，视口又高了：内容跟着下来，不留空白。
    assert_eq!(first_row(&rows(30), tall, &mut view, true), 20);
}

#[test]
fn a_wider_window_rewraps_and_does_not_hold_the_old_row_numbers() {
    let narrow = Rect::new(0, 0, 40, 30);
    let wide = Rect::new(0, 0, 120, 60);
    let mut view = BodyView::default();
    assert_eq!(first_row(&rows(200), narrow, &mut view, true), 170);
    // 最大化：宽了，200 行折成 100 行，视口高了一倍。照最新的露满，不只剩一行。
    assert_eq!(first_row(&rows(100), wide, &mut view, true), 40);
}

#[test]
fn short_content_stays_at_the_top_when_the_view_gets_shorter() {
    let tall = Rect::new(0, 0, 10, 20);
    let short = Rect::new(0, 0, 10, 17);
    let mut view = BodyView::default();
    assert_eq!(first_row(&rows(3), tall, &mut view, true), 0);
    // 开始回答，运行状态行那一块长出来，视口矮了 3 行：放得下的照旧从第一行露起。
    assert_eq!(first_row(&rows(3), short, &mut view, true), 0);
}

#[test]
fn the_end_of_a_turn_lets_the_rows_settle_once() {
    let area = Rect::new(0, 0, 10, 10);
    let mut view = BodyView::default();
    assert_eq!(first_row(&rows(30), area, &mut view, true), 20);
    // 思考、工具收成一行，少了 8 行：回答进行中不动。
    assert_eq!(first_row(&rows(22), area, &mut view, true), 20);
    // 一轮结束：放开一次，上面的行补满空白。
    view.settle();
    assert_eq!(first_row(&rows(22), area, &mut view, true), 12);
}

#[test]
fn a_long_reply_stops_at_its_first_row_until_her_next_step() {
    let area = Rect::new(0, 0, 10, 10);
    let mut view = BodyView::default();
    // 她的正文（编号 7）从第 15 行起。
    view.reading = Some((7, 15));
    assert_eq!(
        first_row(&rows(20), area, &mut view, true),
        10,
        "放得下：照旧跟着"
    );
    assert_eq!(
        first_row(&rows(26), area, &mut view, true),
        15,
        "开头顶到最上面：停住"
    );
    assert_eq!(
        first_row(&rows(40), area, &mut view, true),
        15,
        "后面的字在下面长"
    );
    // 她开始下一步：回到最底下接着跟。
    view.resume();
    assert_eq!(first_row(&rows(42), area, &mut view, true), 32);
    // 下一段正文长了，又停在它的开头。
    view.reading = Some((9, 44));
    assert_eq!(first_row(&rows(60), area, &mut view, true), 44);
}

#[test]
fn scrolled_back_to_the_bottom_the_same_reply_does_not_stop_again() {
    let area = Rect::new(0, 0, 10, 10);
    let mut view = BodyView::default();
    view.area = area;
    view.reading = Some((7, 15));
    view.first = first_row(&rows(30), area, &mut view, true);
    assert_eq!(view.first, 15);
    // 人往下翻到底：跟着最新的，这一段不再停。
    while view.top.is_some() {
        view.page(true);
        view.first = first_row(&rows(30), area, &mut view, true);
    }
    assert_eq!(first_row(&rows(36), area, &mut view, true), 26);
    // 人自己滚上去的：她开始下一步也不动。
    view.page(false);
    view.first = first_row(&rows(36), area, &mut view, true);
    let kept = view.first;
    view.resume();
    assert_eq!(first_row(&rows(40), area, &mut view, true), kept);
}

#[test]
fn with_the_cap_the_end_of_a_turn_does_not_drop_the_rows() {
    let running = Rect::new(0, 0, 10, 10);
    let idle = Rect::new(0, 0, 10, 12);
    let mut view = BodyView::default();
    assert_eq!(first_row(&rows(13), running, &mut view, true), 3);
    // 一轮结束，运行状态行那一块收起，视口长高两行：开着封顶时不往下落，多出的两行空着。
    view.hold();
    assert_eq!(first_row(&rows(13), idle, &mut view, true), 3);
    assert_eq!(
        first_row(&rows(13), idle, &mut view, true),
        3,
        "下一帧照旧不动"
    );
    // 没按住的：贴着底边落两行（命令列表关了、输入框变矮了照这样）。
    let mut view = BodyView::default();
    assert_eq!(first_row(&rows(13), running, &mut view, true), 3);
    assert_eq!(first_row(&rows(13), idle, &mut view, true), 1);
}

#[test]
fn ctrl_l_empties_the_view_and_keeps_the_rows() {
    let area = Rect::new(0, 0, 10, 10);
    let mut view = BodyView::default();
    view.rows = rows(30);
    view.clear();
    // 30 行全顶上去，视口是空的；往回滚还在。
    assert_eq!(first_row(&rows(30), area, &mut view, true), 30);
    // 新的字从空着的视口顶上往下长。
    assert_eq!(first_row(&rows(33), area, &mut view, true), 30);
    view.top = Some(0);
    assert_eq!(first_row(&rows(33), area, &mut view, true), 0);
}

#[test]
fn after_ctrl_l_a_shorter_view_does_not_push_new_rows_up() {
    let tall = Rect::new(0, 0, 10, 10);
    let short = Rect::new(0, 0, 10, 7);
    let mut view = BodyView::default();
    view.rows = rows(30);
    view.clear();
    assert_eq!(first_row(&rows(30), tall, &mut view, true), 30);
    // 发出一句话（4 行），运行状态行那一块长出来，视口矮了 3 行：还从清屏那一行露起，不切掉开头。
    assert_eq!(first_row(&rows(34), short, &mut view, true), 30);
    assert_eq!(first_row(&rows(35), short, &mut view, true), 30);
    // 一轮结束，那一块收起：照旧从清屏那一行露起。
    assert_eq!(first_row(&rows(35), tall, &mut view, true), 30);
    // 新的字多到放不下了，才贴着底边走。
    assert_eq!(first_row(&rows(45), short, &mut view, true), 38);
}

#[test]
fn page_keys_move_half_a_screen() {
    let area = Rect::new(0, 0, 10, 10);
    let mut view = BodyView::default();
    view.area = area;
    // 画的时候会把露出的第一行记进 `view.first`，这里照做。
    let draw = |view: &mut BodyView| {
        view.first = first_row(&rows(50), area, view, true);
        view.first
    };
    assert_eq!(draw(&mut view), 40);
    view.page(false);
    assert_eq!(draw(&mut view), 35, "翻半屏");
    view.page(true);
    assert_eq!(draw(&mut view), 40);
    assert!(view.top.is_none(), "翻到底又跟着最新的");
}

#[test]
fn scrolling_after_ctrl_l_does_not_bring_the_rows_back() {
    let area = Rect::new(0, 0, 10, 10);
    let mut view = BodyView::default();
    view.rows = rows(30);
    view.clear();
    assert_eq!(first_row(&rows(30), area, &mut view, true), 30);
    // 往下滚：还是空的。
    view.top = Some(33);
    assert_eq!(first_row(&rows(30), area, &mut view, true), 30);
    // 往上滚三行：看得到之前的最后三行。
    view.top = Some(27);
    assert_eq!(first_row(&rows(30), area, &mut view, true), 27);
    // 滚回底：又是空的。
    view.top = Some(30);
    assert_eq!(first_row(&rows(30), area, &mut view, true), 30);
}

#[test]
fn a_relayout_spread_over_frames_does_not_strand_the_view_at_the_end() {
    // 2026-10-02 项目主人报：最大化以后内容闪一下就没了，只剩最后一行。长会话整份重排分几帧做，没排到的先用旧行
    // （窄的、行多），「只往下走」记住了那时的底边，排完行少了，视口停在末尾。
    let blank = rows(1).get(0).unwrap().clone();
    let chunks = |a: usize, b: usize| {
        crate::ui::row_cache::Rows::of_chunks(
            &[
                (a, Some(0)),
                (1, None),
                (b, Some(1)),
                (1, None),
                (2, Some(2)),
            ],
            &blank,
        )
    };
    let mut view = BodyView::default();
    let half = Rect::new(0, 0, 40, 10);
    let max = Rect::new(0, 0, 80, 10);
    view.rows = chunks(40, 40);
    view.area = half;
    // 变宽的第一帧：最后一条排好了，前两条还是旧行。
    let mixed = chunks(40, 40);
    let first = first_row(&mixed, max, &mut view, false);
    view.rows = mixed;
    assert_eq!(first, 84 - 10);
    // 下一帧排完了：行少了，照新的露最底下一截，不停在末尾。
    let done = chunks(10, 10);
    let first = first_row(&done, max, &mut view, true);
    assert_eq!(first, 24 - 10, "露最底下一截");
}
