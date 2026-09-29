//! 输入框的测试：编辑按字走、折行按显示宽度、鼠标坐标换回下标。

use std::time::Duration;

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use super::{Action, Editor, InputBox, VisualLine, locate, offset_at, pieces, tail_pieces, wrap};

fn typed(text: &str) -> Editor {
    let mut e = Editor::default();
    e.insert(text);
    e
}

fn input(width: u16) -> InputBox {
    let mut i = InputBox::new(8, Duration::from_millis(400));
    i.place(Rect::new(10, 5, width, 8));
    i
}

fn press(i: &mut InputBox, code: KeyCode, modifiers: KeyModifiers) -> Action {
    i.key(KeyEvent::new(code, modifiers))
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn backspace_removes_a_whole_emoji() {
    // 带肤色的 emoji 是两个字符、一个字素簇。
    let mut e = typed("a👍🏽");
    e.backspace();
    assert_eq!(e.text(), "a");
}

#[test]
fn typing_replaces_the_selection() {
    let mut e = typed("你好世界");
    e.select("你".len(), "你好".len());
    e.insert("们");
    assert_eq!(e.text(), "你们世界");
    assert_eq!(e.selection(), None);
}

#[test]
fn left_collapses_the_selection_to_its_start() {
    let mut e = typed("abcd");
    e.select(1, 3);
    e.left(false);
    assert_eq!((e.cursor(), e.selection()), (1, None));
}

#[test]
fn paste_normalizes_line_endings_tabs_and_controls() {
    let e = typed("a\r\nb\tc\u{7}");
    assert_eq!(e.text(), "a\nb    c");
}

#[test]
fn wrap_counts_chinese_as_two_columns() {
    // 宽 5：两个汉字 4 列，第三个放不下。
    let lines = wrap("你好世界", 5);
    assert_eq!(
        lines,
        vec![
            VisualLine { start: 0, end: 6 },
            VisualLine { start: 6, end: 12 },
        ]
    );
}

#[test]
fn empty_text_and_trailing_newline_still_have_a_line() {
    assert_eq!(wrap("", 10).len(), 1);
    assert_eq!(wrap("a\n", 10).len(), 2);
}

#[test]
fn cursor_at_a_soft_wrap_sits_on_the_next_line() {
    let text = "abcdef";
    let lines = wrap(text, 3);
    assert_eq!(locate(text, &lines, 3), (1, 0));
    // 硬换行前的行尾算在本行。
    let text = "abc\ndef";
    let lines = wrap(text, 10);
    assert_eq!(locate(text, &lines, 3), (0, 3));
}

#[test]
fn clicking_a_wide_char_picks_the_nearer_side() {
    let text = "你好";
    let lines = wrap(text, 10);
    assert_eq!(offset_at(text, &lines, 0, 0), 0);
    assert_eq!(offset_at(text, &lines, 0, 1), "你".len());
    assert_eq!(offset_at(text, &lines, 0, 9), text.len());
}

#[test]
fn up_and_down_keep_the_goal_column() {
    let mut i = input(20);
    i.paste("abcdef\nab\nabcdef");
    // 光标在最后，第 2 行第 6 列；往上经过短行，再往上回到第 6 列。
    press(&mut i, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(i.editor.cursor(), "abcdef\nab".len());
    press(&mut i, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(i.editor.cursor(), "abcdef".len());
}

#[test]
fn enter_submits_and_shift_enter_breaks_the_line() {
    let mut i = input(20);
    i.paste("hi");
    press(&mut i, KeyCode::Enter, KeyModifiers::SHIFT);
    i.paste("there");
    let sent = press(&mut i, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(sent, Action::Submit("hi\nthere".into()));
    assert!(i.editor.is_empty());
    // 只有空白的不发。
    i.paste("  \n ");
    assert_eq!(
        press(&mut i, KeyCode::Enter, KeyModifiers::NONE),
        Action::None
    );
}

#[test]
fn ctrl_c_copies_then_clears_then_only_hints() {
    let mut i = input(20);
    i.paste("abc");
    press(&mut i, KeyCode::Char('a'), KeyModifiers::CONTROL);
    let ctrl_c = |i: &mut InputBox| press(i, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(ctrl_c(&mut i), Action::Copy("abc".into()));
    press(&mut i, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(ctrl_c(&mut i), Action::None);
    assert!(i.editor.is_empty());
    assert_eq!(
        ctrl_c(&mut i),
        Action::ExitHint,
        "空着不退出，只提示用 Ctrl+D"
    );
}

#[test]
fn ctrl_s_stashes_restores_and_swaps() {
    let mut i = input(20);
    let ctrl_s = |i: &mut InputBox| press(i, KeyCode::Char('s'), KeyModifiers::CONTROL);
    i.paste("草稿");
    ctrl_s(&mut i);
    assert!(i.editor.is_empty() && i.stashed(), "有字：存起来、清空");
    i.paste("另一句");
    ctrl_s(&mut i);
    assert_eq!(i.editor.text(), "草稿", "两边都有：互换");
    assert!(i.stashed());
    i.editor.take();
    ctrl_s(&mut i);
    assert_eq!(i.editor.text(), "另一句", "空着：取回来");
    assert!(!i.stashed());
}

#[test]
fn dragging_selects_and_ctrl_c_copies() {
    let mut i = input(20);
    i.paste("hello world");
    // 文字区左上角在 (10, 5)。
    i.mouse(mouse(MouseEventKind::Down(MouseButton::Left), 10, 5), true);
    i.mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 15, 5), true);
    let up = i.mouse(mouse(MouseEventKind::Up(MouseButton::Left), 15, 5), true);
    assert_eq!(up, Action::None, "松开不复制");
    assert_eq!(selected(&i), Some("hello"), "选区留着");
    let copied = press(&mut i, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(copied, Action::Copy("hello".into()));
}

#[test]
fn double_click_selects_a_word() {
    let mut i = input(20);
    i.paste("hello world");
    let down = mouse(MouseEventKind::Down(MouseButton::Left), 17, 5);
    i.mouse(down, true);
    i.mouse(mouse(MouseEventKind::Up(MouseButton::Left), 17, 5), true);
    assert_eq!(i.mouse(down, true), Action::None, "双击只选中，不复制");
    assert_eq!(selected(&i), Some("world"));
}

#[test]
fn a_click_outside_the_box_does_nothing() {
    let mut i = input(20);
    i.paste("abc");
    i.mouse(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0), false);
    assert_eq!(i.editor.cursor(), 3);
}

#[test]
fn pieces_know_which_lines_were_folded() {
    // 宽 3：「abcdef」折成两行，第二行是折下来的；换行符后面的那一行不是。
    assert_eq!(
        pieces("abcdef\ngh", 3),
        vec![
            ("abc".into(), false),
            ("def".into(), true),
            ("gh".into(), false)
        ]
    );
}

#[test]
fn word_jumps_and_word_delete() {
    let mut e = typed("cargo test  --lib");
    e.word_left(false);
    assert_eq!(&e.text()[e.cursor()..], "lib");
    e.word_left(false);
    assert_eq!(&e.text()[e.cursor()..], "--lib", "连着的标点算一个词");
    e.move_to(0, false);
    e.word_right(false);
    assert_eq!(e.cursor(), "cargo".len());
    let mut e = typed("cargo test  ");
    e.delete_word();
    assert_eq!(e.text(), "cargo ", "连同后面的空白一起删");
}

#[test]
fn a_run_of_chinese_is_one_word() {
    let mut e = typed("中文，测试");
    e.delete_word();
    assert_eq!(e.text(), "中文，", "一串中文到标点为止算一个词");
    e.delete_word();
    assert_eq!(e.text(), "中文");
    e.delete_word();
    assert_eq!(e.text(), "");
    let mut e = typed("你好 世界 ok");
    e.move_to(0, false);
    e.word_right(false);
    assert_eq!(&e.text()[..e.cursor()], "你好");
    e.word_right(false);
    assert_eq!(&e.text()[..e.cursor()], "你好 世界");
    e.word_left(false);
    assert_eq!(&e.text()[e.cursor()..], "世界 ok");
    assert_eq!(
        e.word_at("你好 ".len() + 3),
        ("你好 ".len(), "你好 世界".len()),
        "双击选同一串"
    );
}

#[test]
fn up_on_the_first_line_browses_history_and_down_returns_the_draft() {
    let mut i = input(20);
    i.remember("第一句".into());
    i.remember("第二句".into());
    i.paste("没发的");
    press(&mut i, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(i.editor.text(), "第二句");
    press(&mut i, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(i.editor.text(), "第一句");
    press(&mut i, KeyCode::Up, KeyModifiers::NONE);
    assert_eq!(i.editor.text(), "第一句", "最旧的再往上不动");
    press(&mut i, KeyCode::Down, KeyModifiers::NONE);
    press(&mut i, KeyCode::Down, KeyModifiers::NONE);
    assert_eq!(i.editor.text(), "没发的", "翻过最新一条回到没发的那句");
}

#[test]
fn an_undone_line_comes_back_selected_and_typing_replaces_it() {
    let mut i = input(40);
    i.put_back("说一句话就好");
    assert_eq!(selected(&i), Some("说一句话就好"));
    // 直接打 /redo：替换掉那句，不拼在后面。
    i.paste("/redo");
    assert_eq!(i.editor.text(), "/redo");
    // 框里已经有字的不动。
    let mut busy = input(40);
    busy.paste("草稿");
    busy.put_back("说一句话就好");
    assert_eq!(busy.editor.text(), "草稿");
}

#[test]
fn redo_takes_back_the_line_only_if_untouched() {
    let mut i = input(40);
    i.put_back("说一句话就好");
    i.take_back();
    assert!(i.editor.is_empty(), "没动过的收回去，免得回车再发一遍");
    let mut edited = input(40);
    edited.put_back("说一句话就好");
    press(&mut edited, KeyCode::End, KeyModifiers::NONE);
    edited.paste("，再说一句");
    edited.take_back();
    assert_eq!(edited.editor.text(), "说一句话就好，再说一句", "改过的留着");
}

#[test]
fn picking_from_history_keeps_what_was_in_the_box() {
    let mut i = input(40);
    i.remember("跑一下测试".into());
    i.paste("写到一半");
    i.pick("跑一下测试");
    assert_eq!(i.editor.text(), "跑一下测试");
    assert!(i.stashed(), "原来的字存进暂存");
    // 暂存里已经有字了：原来的字记进输入历史，不丢。
    i.editor.set("又写了一句");
    i.pick("跑一下测试");
    assert_eq!(i.sent().last().map(String::as_str), Some("又写了一句"));
}

#[test]
fn tail_pieces_match_the_end_of_a_full_wrap() {
    // 只折最后几行（思考的预览，`tui.md`「时间线」第 5 条）：和整段折完取最后几行一模一样，连「折下来的」标记。
    let texts = [
        "",
        "一行",
        "a\n\nb\n",
        "很长的一段中文没有换行一直写下去直到超过宽度好几倍为止看看折得对不对",
        "第一段\n第二段比较长比较长比较长比较长比较长\n\n第三段\nlast line with english words",
    ];
    for text in texts {
        for width in [1u16, 4, 9, 30] {
            let full = pieces(text, width);
            for rows in [0usize, 1, 3, 15, 100] {
                let want = full[full.len().saturating_sub(rows)..].to_vec();
                assert_eq!(
                    tail_pieces(text, width, rows),
                    want,
                    "{text:?} 宽 {width} 取 {rows}"
                );
            }
        }
    }
}

mod paste;

/// 选中的字（没选中是 `None`）。
fn selected(i: &InputBox) -> Option<&str> {
    i.editor.selection().map(|(s, e)| &i.editor.text()[s..e])
}
