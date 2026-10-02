//! 版面（蓝图 `tui.md`「运行状态行和排队的消息」「输入框」「窗口小的时候」）：各块摆在哪。

use std::time::Duration;

use ratatui::layout::Rect;

use super::areas;
use crate::config::Config;
use crate::input::InputBox;

#[test]
fn while_answering_one_blank_above_the_status_and_one_below_it() {
    let layout = Config::builtin().unwrap().layout;
    let input = InputBox::new(8, Duration::from_millis(400));
    let a = areas(
        Rect::new(0, 0, 100, 40),
        &|w| input.rows(w),
        &layout,
        0,
        true,
        2,
        0,
        0,
    );
    // 正文 · 空一行 · 运行状态行 · 两条排队的 · 空一行 · 输入框（`tui.md`「运行状态行和排队的消息」）。
    assert_eq!(a.pulse.y, a.body.bottom() + 1, "上面空一行");
    assert_eq!((a.queued.y, a.queued.height), (a.pulse.y + 1, 2));
    assert_eq!(a.frame.y, a.queued.bottom() + 1, "下面空一行");
    // 没有排队的：空行直接在运行状态行下面。
    let b = areas(
        Rect::new(0, 0, 100, 40),
        &|w| input.rows(w),
        &layout,
        0,
        true,
        0,
        0,
        0,
    );
    assert_eq!(b.frame.y, b.pulse.y + 2);
    // 没在回答：只空一行。
    let c = areas(
        Rect::new(0, 0, 100, 40),
        &|w| input.rows(w),
        &layout,
        0,
        false,
        0,
        0,
        0,
    );
    assert_eq!(c.frame.y, c.body.bottom() + 1);
}

#[test]
fn the_narrow_todo_list_sits_above_the_box_with_a_blank_between() {
    let layout = Config::builtin().unwrap().layout;
    let input = InputBox::new(8, Duration::from_millis(400));
    // 没在回答：正文 · 空一行 · 待办 6 行 · 空一行 · 输入框。
    let a = areas(
        Rect::new(0, 0, 100, 40),
        &|w| input.rows(w),
        &layout,
        0,
        false,
        0,
        0,
        6,
    );
    assert_eq!((a.todo.height, a.todo.bottom() + 1), (6, a.frame.y));
    assert_eq!(a.todo.y, a.body.bottom() + 1);
    assert_eq!(a.todo.x, a.text.x, "和框里的字左对齐");
    // 开着列表：列表是覆盖层，贴着输入框盖在待办上，待办不挪（2026-10-02）。
    let b = areas(
        Rect::new(0, 0, 100, 40),
        &|w| input.rows(w),
        &layout,
        4,
        false,
        0,
        0,
        6,
    );
    assert_eq!(b.menu.bottom(), b.frame.y);
    assert_eq!(b.todo, a.todo, "待办不挪");
    // 在回答：运行状态行那一块在待办上面。
    let c = areas(
        Rect::new(0, 0, 100, 40),
        &|w| input.rows(w),
        &layout,
        0,
        true,
        0,
        0,
        6,
    );
    assert_eq!(c.pulse.y + 2, c.todo.y);
}

#[test]
fn a_narrow_window_drops_the_margins() {
    // 少于 80 列换紧凑版面：框贴着两边，`❯` 紧挨左边线，右边只留光标一列（`tui.md`「输入框」第 10 条）。
    let layout = Config::builtin().unwrap().layout;
    let input = InputBox::new(8, Duration::from_millis(400));
    let rows = |w: u16| input.rows(w);
    let a = areas(Rect::new(0, 0, 44, 30), &rows, &layout, 0, false, 0, 0, 0);
    assert_eq!((a.frame.x, a.frame.width), (0, 44), "框贴着两边");
    assert_eq!(a.text.x, 3, "边线一列，提示符两列");
    assert_eq!(a.text.width, 44 - 3 - 2, "右边只留光标一列和边线");
    assert_eq!(a.footer.x, a.text.x, "框下面那一行跟着字对齐");
    let h = super::home::areas(Rect::new(0, 0, 44, 30), &rows, &layout, (0, 0), 0, 0, 0);
    assert_eq!((h.frame.x, h.text.x, h.text.width), (0, 3, 39), "首页一样");
    // 79 列还是紧凑的；80 列起照旧留白。
    let c = areas(Rect::new(0, 0, 79, 30), &rows, &layout, 0, false, 0, 0, 0);
    assert_eq!((c.frame.x, c.text.x), (0, 3), "79 列紧凑");
    let b = areas(Rect::new(0, 0, 80, 30), &rows, &layout, 0, false, 0, 0, 0);
    assert_eq!(b.text.x, b.frame.x + 1 + layout.pad_left);
    assert!(b.frame.x >= layout.side_gap);
}

#[test]
fn the_home_screen_always_shows_the_level_tip() {
    let config = Config::builtin().unwrap();
    // 首页：固定这一条，不管轮到哪一条（`tui.md`「输入框」第 9 条）。
    assert_eq!(
        super::input_box::placeholder(&config, true, 3),
        "Tab 切换权限级别"
    );
    // 离开首页：照轮换的。
    assert_eq!(
        super::input_box::placeholder(&config, false, 3),
        config.text.tips[3]
    );
}

#[test]
fn nothing_above_the_box_covers_it_in_a_short_window() {
    // 2026-09-29 30×7 实测：命令列表压到输入框的上边框上（`tui.md`「窗口小的时候」第 1 条）。
    let layout = Config::builtin().unwrap().layout;
    let input = InputBox::new(8, Duration::from_millis(400));
    let rows = |w: u16| input.rows(w);
    let a = areas(Rect::new(0, 0, 30, 7), &rows, &layout, 6, false, 0, 0, 3);
    assert_eq!(a.menu.bottom(), a.frame.y, "列表贴着输入框");
    assert_eq!(a.menu.height, a.frame.y, "只用输入框上面剩下的行");
    for (name, r) in [("列表", a.menu), ("待办", a.todo)] {
        assert!(
            !r.intersects(a.frame),
            "{name}盖住了输入框：{r:?} {:?}",
            a.frame
        );
    }
    // 在回答、还排着两条：运行状态行那一块放不下的不盖输入框。
    let b = areas(Rect::new(0, 0, 30, 6), &rows, &layout, 0, true, 2, 0, 0);
    for (name, r) in [("运行状态行", b.pulse), ("排队的", b.queued)] {
        assert!(
            !r.intersects(b.frame),
            "{name}盖住了输入框：{r:?} {:?}",
            b.frame
        );
    }
    // 首页：输入框上面没地方时整组往下挪，给列表腾地方；挪到底也不盖。
    let h = super::home::areas(Rect::new(0, 0, 30, 7), &rows, &layout, (0, 0), 6, 0, 0);
    assert!(!h.menu.intersects(h.frame), "首页的列表盖住了输入框");
    assert!(h.menu.height >= 1, "首页挪出至少一行给列表");
    assert!(h.footer.bottom() <= 7);
}

#[test]
fn the_lists_sit_in_a_frame_lined_up_with_the_input_box() {
    // 2026-09-30 项目主人：输入框上面三样一个框。选中的 ❯ 和提示符同一列、名字和打的字同一列；首页的输入框窄，
    // 框里的几条照这一帧量出来的宽度排（原来照正文的宽度排，首页后台面板右边的状态被截掉）。
    let layout = Config::builtin().unwrap().layout;
    let input = InputBox::new(8, Duration::from_millis(400));
    let rows = |w: u16| input.rows(w);
    let lead = u16::try_from(unicode_width::UnicodeWidthStr::width(
        layout.prompt.as_str(),
    ))
    .unwrap();
    let area = Rect::new(0, 0, 160, 40);
    for (name, a) in [
        ("正文", areas(area, &rows, &layout, 7, false, 0, 0, 0)),
        (
            "首页",
            super::home::areas(area, &rows, &layout, (0, 0), 7, 0, 0),
        ),
    ] {
        assert_eq!(
            (a.menu.x, a.menu.width),
            (a.frame.x, a.frame.width),
            "{name}：框和输入框的边对齐"
        );
        assert_eq!(a.menu_text.x, a.text.x - lead, "{name}：❯ 和提示符同一列");
        assert_eq!(
            a.menu_text.right(),
            a.text.right(),
            "{name}：右边和框里的字对齐"
        );
        assert!(
            a.menu_text.x > a.menu.x && a.menu_text.right() < a.menu.right(),
            "{name}：字在框里"
        );
        assert_eq!(
            (a.menu_text.y, a.menu_text.height),
            (a.menu.y + 1, a.menu.height - 2),
            "{name}：上下让出两条边"
        );
    }
}

#[test]
fn the_agent_circles_line_up_with_the_level_icon() {
    // 2026-09-30 项目主人：● ○ 和框下面那一行的 ▣ 同一列，焦点的 ❯ 和提示符同一列（原来圆圈靠右两格）。
    let layout = Config::builtin().unwrap().layout;
    let input = InputBox::new(8, Duration::from_millis(400));
    let rows = |w: u16| input.rows(w);
    let lead = u16::try_from(unicode_width::UnicodeWidthStr::width(
        layout.prompt.as_str(),
    ))
    .unwrap();
    let area = Rect::new(0, 0, 160, 40);
    for (name, a) in [
        ("正文", areas(area, &rows, &layout, 0, false, 0, 4, 0)),
        (
            "首页",
            super::home::areas(area, &rows, &layout, (0, 0), 0, 4, 0),
        ),
    ] {
        assert_eq!(a.agents.x + lead, a.footer.x, "{name}：圆圈和 ▣ 同一列");
        assert_eq!(a.agents.right(), a.footer.right(), "{name}：右边照旧");
    }
}

#[test]
fn an_open_list_covers_the_body_instead_of_pushing_it_up() {
    // 2026-10-02 项目主人：输入框上面的框改成覆盖层，盖在正文底部上；原来把正文往上推，高度一变正文就上下跳。
    let layout = Config::builtin().unwrap().layout;
    let input = InputBox::new(8, Duration::from_millis(400));
    let rows = |w: u16| input.rows(w);
    let area = Rect::new(0, 0, 100, 40);
    for running in [false, true] {
        let shut = areas(area, &rows, &layout, 0, running, 2, 0, 3);
        let open = areas(area, &rows, &layout, 7, running, 2, 0, 3);
        assert_eq!(open.body, shut.body, "正文一行不动（在回答：{running}）");
        assert_eq!(
            (open.pulse, open.queued, open.todo),
            (shut.pulse, shut.queued, shut.todo),
            "运行状态行、排队的、待办照没开框时的位置"
        );
        assert_eq!(open.menu.bottom(), open.frame.y, "框贴着输入框");
        assert_eq!(open.menu.height, 7);
        if !running {
            assert!(open.menu.intersects(open.body), "盖在正文底部上");
        }
    }
}
