//! 找回来（蓝图 `tui.md`「按键」`Ctrl+C`、「输入框」第 7 条）：`Ctrl+C` 清掉的那句留最近一份、`↑` 找回；撤销放回的
//! 那句照发出去时的样子，长文、附件还是块。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::attach_rule;
use super::paste::{folding, submit};
use crate::input::{Action, Draft, InputBox};

fn press(i: &mut InputBox, code: KeyCode, modifiers: KeyModifiers) -> Action {
    i.key(KeyEvent::new(code, modifiers))
}

/// 一段带着长文块和附件的话：交回输入框和那个文件。
fn busy_draft(tag: &str) -> (InputBox, std::path::PathBuf) {
    let file = std::env::temp_dir().join(format!("miyu-recover-{tag}-{}.png", std::process::id()));
    std::fs::write(&file, b"x").unwrap();
    let mut i = folding();
    i.set_attach_rule(attach_rule());
    i.editor.insert("写了半天：");
    i.paste(
        &(1..=12)
            .map(|n| format!("第 {n} 行"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    i.paste(&file.display().to_string());
    (i, file)
}

#[test]
fn a_ctrl_c_clear_is_kept_once_outside_the_history_and_up_brings_it_back() {
    // 2026-09-30 项目主人：误按 Ctrl+C，辛苦打的字全没了；清掉的别进历史列表，不然每清一次都污染它。
    let ctrl_c = |i: &mut InputBox| press(i, KeyCode::Char('c'), KeyModifiers::CONTROL);
    let up = |i: &mut InputBox| press(i, KeyCode::Up, KeyModifiers::NONE);
    let (mut i, file) = busy_draft("clear");
    i.remember(Draft::plain("发过的一句"));
    let before = i.draft();
    assert_eq!(ctrl_c(&mut i), Action::Cleared);
    assert!(i.editor.is_empty());
    assert_eq!(i.sent().len(), 1, "不进输入历史");
    up(&mut i);
    assert_eq!(i.draft(), before, "↑ 先拿回清掉的，长文、附件还是块");
    assert_eq!(i.draft().attachments(), std::slice::from_ref(&file));
    i.editor.move_to(0, false);
    up(&mut i);
    assert_eq!(i.editor.text(), "发过的一句", "再按才进历史");
    // 只留最近一份：清两次，拿回的是后一次的。
    let mut i = folding();
    i.remember(Draft::plain("发过的一句"));
    i.editor.insert("甲");
    ctrl_c(&mut i);
    i.editor.insert("乙");
    ctrl_c(&mut i);
    up(&mut i);
    assert_eq!(i.editor.text(), "乙");
    // 发出去一句话以后丢掉：↑ 照旧先翻到发过的。
    i.editor.take();
    i.editor.insert("丙");
    ctrl_c(&mut i);
    i.forget_cleared();
    up(&mut i);
    assert_eq!(i.editor.text(), "发过的一句");
    std::fs::remove_file(&file).unwrap_or_default();
}

#[test]
fn an_undone_line_comes_back_as_it_was_sent_and_goes_again_on_restore() {
    // 2026-09-30 项目主人：撤销放回的那句带回附件和长文块。核心给的是发出去的全文。
    let (mut i, file) = busy_draft("undo");
    let sent = submit(&mut i);
    i.remember(sent.clone());
    i.put_back(&sent.expand());
    assert_eq!(i.draft(), sent, "照发出去时的样子");
    assert_eq!(i.editor.selection(), Some((0, sent.text.len())), "整段选中");
    i.take_back();
    assert!(i.editor.is_empty(), "没动过的，恢复时收回去");
    // 输入历史里找不到的：照核心给的字放。
    let mut other = folding();
    other.put_back("说一句话就好");
    assert_eq!(other.editor.text(), "说一句话就好");
    std::fs::remove_file(&file).unwrap_or_default();
}
