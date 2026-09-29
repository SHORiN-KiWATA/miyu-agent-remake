//! 大段粘贴收成一块（蓝图 `tui.md`「输入框」第 11 条）：门槛、当一个字、按位置记着、发出去是全文、历史和暂存连块一起、
//! 自己打的不算、退回的消息、单独的 `\r`、不留选中、复制是全文。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::input;
use crate::input::{Action, InputBox, PasteRule};

/// 粘贴收成一块的门槛和写法（照出厂配置的样子）：超过 10 行或 800 字。
fn folding() -> InputBox {
    let mut i = input(40);
    i.set_paste_rule(PasteRule {
        lines: 10,
        chars: 800,
        label: "[已粘贴 {lines} 行]".into(),
    });
    i
}

fn lines_of(n: usize) -> String {
    (1..=n)
        .map(|i| format!("第 {i} 行"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn submit(i: &mut InputBox) -> crate::input::Draft {
    match i.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
        Action::Submit(draft) => draft,
        other => panic!("没发出去：{other:?}"),
    }
}

#[test]
fn a_big_paste_becomes_one_block() {
    // 超过 10 行或 800 字才收（`tui.md`「输入框」第 11 条）；刚好 10 行、800 字的照原文放。
    let mut i = folding();
    i.paste(&lines_of(10));
    assert_eq!(i.editor.text(), lines_of(10));
    let mut i = folding();
    i.paste(&lines_of(11));
    assert_eq!(i.editor.text(), "[已粘贴 11 行]", "不编号");
    i.paste(&"字".repeat(800));
    assert!(i.editor.text().ends_with(&"字".repeat(800)), "800 字不收");
    i.paste(&"长".repeat(801));
    assert!(
        i.editor.text().ends_with("[已粘贴 1 行]"),
        "{}",
        i.editor.text()
    );
}

#[test]
fn a_block_moves_and_deletes_as_one() {
    let mut i = folding();
    i.editor.insert("看");
    i.paste(&lines_of(12));
    i.editor.insert("吗");
    let label = "[已粘贴 12 行]";
    assert_eq!(i.editor.text(), format!("看{label}吗"));
    let after = "看".len() + label.len();
    // 左移：先过「吗」，再一下跳过整块。
    i.editor.left(false);
    assert_eq!(i.editor.cursor(), after);
    i.editor.left(false);
    assert_eq!(i.editor.cursor(), "看".len());
    // 右移一下跳回块尾；点在块中间落到近的那一头。
    i.editor.right(false);
    assert_eq!(i.editor.cursor(), after);
    i.editor.move_to("看".len() + 3, false);
    assert_eq!(i.editor.cursor(), "看".len(), "离开头近");
    i.editor.move_to(after - 2, false);
    assert_eq!(i.editor.cursor(), after, "离结尾近");
    // 在块尾退格：整块删；块前打字，块跟着挪。
    i.editor.backspace();
    assert_eq!(i.editor.text(), "看吗");
    let mut i = folding();
    i.paste(&lines_of(12));
    i.editor.move_to(0, false);
    i.editor.insert("前面");
    assert_eq!(
        i.editor.blocks(),
        vec![("前面".len(), "前面".len() + label.len())]
    );
    // 在块头 Delete、拖过一半的选区、Ctrl+W：都整块删。
    i.editor.move_to("前面".len(), false);
    i.editor.delete();
    assert_eq!(i.editor.text(), "前面");
    let mut i = folding();
    i.editor.insert("甲");
    i.paste(&lines_of(12));
    i.editor.select(0, "甲".len() + label.len() - 2);
    assert_eq!(i.editor.selection(), Some((0, "甲".len() + label.len())));
    i.editor.backspace();
    assert_eq!(i.editor.text(), "");
    let mut i = folding();
    i.editor.insert("甲 ");
    i.paste(&lines_of(12));
    i.editor.delete_word();
    assert_eq!(i.editor.text(), "甲 ");
    assert!(i.editor.blocks().is_empty());
}

#[test]
fn same_looking_blocks_keep_their_own_text() {
    // 两块都是 12 行，写出来一样，各是各的原文（按位置记，不靠认字）。
    let mut i = folding();
    let a = lines_of(12);
    let b = lines_of(12).replace("行", "条");
    i.paste(&a);
    i.editor.insert(" 和 ");
    i.paste(&b);
    assert_eq!(i.editor.text(), "[已粘贴 12 行] 和 [已粘贴 12 行]");
    let draft = submit(&mut i);
    assert_eq!(draft.expand(), format!("{a} 和 {b}"));
    assert_eq!(
        draft.pasted(),
        vec![
            ("[已粘贴 12 行]".to_string(), a.clone()),
            ("[已粘贴 12 行]".to_string(), b.clone())
        ]
    );
}

#[test]
fn sending_expands_blocks_and_history_keeps_them() {
    let mut i = folding();
    i.editor.insert("看看：");
    i.paste(&lines_of(12));
    let draft = submit(&mut i);
    assert_eq!(draft.text, "看看：[已粘贴 12 行]");
    assert_eq!(
        draft.expand(),
        format!("看看：{}", lines_of(12)),
        "发出去是全文"
    );
    // 记进历史的连块一起；翻回来还是一块，退格整块删。
    i.remember(draft.clone());
    i.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(i.editor.text(), draft.text);
    assert_eq!(i.draft().expand(), draft.expand());
    i.editor.backspace();
    assert_eq!(i.editor.text(), "看看：");
    // 从历史列表挑回来的也是。
    i.pick(&draft.text);
    assert_eq!(i.draft().expand(), draft.expand());
}

#[test]
fn a_typed_look_alike_is_not_a_block() {
    // 自己打出一样的字不算一块，旁边有真的块也一样。
    let mut i = folding();
    i.paste(&lines_of(12));
    i.editor.insert("[已粘贴 12 行]");
    i.editor.backspace();
    assert_eq!(i.editor.text(), "[已粘贴 12 行][已粘贴 12 行");
    assert_eq!(i.editor.blocks().len(), 1);
}

#[test]
fn stashing_keeps_a_block() {
    let mut i = folding();
    i.paste(&lines_of(12));
    let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    i.key(ctrl('s'));
    assert!(i.editor.is_empty());
    i.key(ctrl('s'));
    assert_eq!(i.editor.text(), "[已粘贴 12 行]");
    i.editor.backspace();
    assert!(i.editor.is_empty(), "取回来还是一块");
}

#[test]
fn a_returned_message_comes_back_with_its_blocks() {
    // 被退回的排队消息：正文里记着的样子和原文，放回输入框还是一块。
    let draft = crate::input::Draft::from_pasted(
        "看看：[已粘贴 2 行]",
        &[("[已粘贴 2 行]".into(), "甲\n乙".into())],
    );
    let mut i = folding();
    i.editor.set_draft(draft);
    assert_eq!(i.draft().expand(), "看看：甲\n乙");
    i.editor.backspace();
    assert_eq!(i.editor.text(), "看看：");
}

#[test]
fn a_lone_carriage_return_in_a_paste_is_a_newline() {
    // kitty 这类终端粘贴时把换行送成 `\r`：丢掉的话多行粘成一行，行数也数不对。
    let mut i = input(40);
    i.paste("甲\r乙\r\n丙");
    assert_eq!(i.editor.text(), "甲\n乙\n丙");
}

#[test]
fn pasting_after_a_click_leaves_nothing_selected() {
    // 点一下输入框会记下选区的起点；之后放进来、打进来的字不能变成选中的样子（项目主人截图）。
    let mut i = folding();
    i.editor.select(0, 0);
    i.paste(&lines_of(12));
    assert_eq!(i.editor.selection(), None, "粘贴的整块没被选中");
    i.editor.select(i.editor.cursor(), i.editor.cursor());
    i.editor.insert("甲");
    assert_eq!(i.editor.selection(), None, "打的字没被选中");
}

#[test]
fn copying_a_selection_gives_the_full_text() {
    let mut i = folding();
    i.editor.insert("看");
    i.paste(&lines_of(12));
    i.editor.select_all();
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(i.key(ctrl_c), Action::Copy(format!("看{}", lines_of(12))));
}
