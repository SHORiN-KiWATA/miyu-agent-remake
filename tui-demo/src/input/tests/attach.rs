//! 附件（蓝图 `tui.md`「输入框」第 12 条）：拖进来的文件认种类、收成块，编号每一种各数各的、整个会话一直往下排，
//! 增删一块照先后重编，别的文件照路径写。

use std::collections::HashMap;
use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::attach_rule;
use super::paste::{folding, submit};
use crate::input::InputBox;

/// 临时目录里放几个文件，交回目录和每个文件。
fn files(tag: &str, names: &[&str]) -> (PathBuf, Vec<PathBuf>) {
    let dir = std::env::temp_dir().join(format!("miyu-attach-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let paths = names
        .iter()
        .map(|n| {
            let p = dir.join(n);
            std::fs::write(&p, b"x").unwrap();
            p
        })
        .collect();
    (dir, paths)
}

fn attaching() -> InputBox {
    let mut i = folding();
    i.set_attach_rule(attach_rule());
    i
}

fn backspace(i: &mut InputBox) {
    i.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
}

#[test]
fn dropped_files_become_blocks_by_kind_and_other_files_stay_paths() {
    // 2026-09-30 项目主人：图片、音频、视频、PDF 做成附件；别的文件收成写着文件名的文件块，发出去换回路径。
    let (dir, f) = files(
        "kinds",
        &["a.png", "报告 1.pdf", "b.MP3", "c.mp4", "说明.txt"],
    );
    let mut i = attaching();
    let quoted: Vec<String> = f[..4]
        .iter()
        .map(|p| format!("'{}'", p.display()))
        .collect();
    i.paste(&quoted.join(" "));
    assert_eq!(i.editor.text(), "[图片 1] [PDF 1] [音频 1] [视频 1]");
    let draft = submit(&mut i);
    assert_eq!(draft.attachments(), f[..4].to_vec(), "文件照先后跟着发");
    assert_eq!(
        draft.expand(),
        "[图片 1] [PDF 1] [音频 1] [视频 1]",
        "字里照留块上的字"
    );
    let mut i = attaching();
    i.paste(&f[4].display().to_string());
    assert_eq!(i.editor.text(), "[说明.txt]");
    let draft = submit(&mut i);
    assert_eq!(draft.expand(), f[4].display().to_string(), "发出去是路径");
    assert!(draft.attachments().is_empty(), "不当附件");
    // 目录也是，名字后面带 /；点它打开的是那个目录。
    let mut i = attaching();
    i.paste(&dir.display().to_string());
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let name = crate::input::attach::short_name(&name, 24);
    assert_eq!(i.editor.text(), format!("[{name}/]"));
    let opens: Vec<_> = i
        .editor
        .openable()
        .map(|(_, _, p)| p.to_path_buf())
        .collect();
    assert_eq!(opens, std::slice::from_ref(&dir));
    std::fs::remove_dir_all(&dir).unwrap_or_default();
}

#[test]
fn numbers_follow_the_session_and_close_up_when_a_block_goes() {
    // 2026-09-30 项目主人：编号整个会话一直排，发出去以后不从 1 重来。
    let (dir, f) = files("numbers", &["a.png", "b.jpg", "c.pdf"]);
    let mut i = attaching();
    i.renumber(HashMap::from([("image".to_string(), 2)]));
    i.editor.insert("看");
    i.paste(&f[0].display().to_string());
    i.paste(&f[1].display().to_string());
    i.paste(&f[2].display().to_string());
    assert_eq!(i.editor.text(), "看[图片 3][图片 4][PDF 1]");
    // 删掉中间一块：后面的照先后重编，不会出两个一样的号。
    let at = i.editor.text().find("[PDF 1]").unwrap();
    i.editor.move_to(at, false);
    backspace(&mut i);
    assert_eq!(i.editor.text(), "看[图片 3][PDF 1]");
    i.paste(&f[0].display().to_string());
    assert_eq!(i.editor.text(), "看[图片 3][图片 4][PDF 1]");
    // 正文里又多了几个（比如撤销的恢复了）：输入框里的跟着挪号，光标、后面的块跟着挪。
    let end = i.editor.text().len();
    i.editor.move_to(end, false);
    i.renumber(HashMap::from([("image".to_string(), 9)]));
    assert_eq!(i.editor.text(), "看[图片 10][图片 11][PDF 1]");
    assert_eq!(i.editor.cursor(), i.editor.text().len(), "光标还在最后");
    assert_eq!(i.editor.blocks().len(), 3);
    backspace(&mut i);
    assert_eq!(
        i.editor.text(),
        "看[图片 10][图片 11]",
        "块的位置跟着挪对了：退格整块删"
    );
    std::fs::remove_dir_all(&dir).unwrap_or_default();
}

#[test]
fn a_draft_from_history_gets_the_numbers_of_now() {
    let (dir, f) = files("history", &["a.png"]);
    let mut i = attaching();
    i.paste(&f[0].display().to_string());
    let sent = submit(&mut i);
    assert_eq!(sent.text, "[图片 1]");
    i.remember(sent);
    // 发出去以后正文里有了一张：翻回来的那句照现在的编号。
    i.renumber(HashMap::from([("image".to_string(), 1)]));
    i.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    i.renumber(HashMap::from([("image".to_string(), 1)]));
    assert_eq!(i.editor.text(), "[图片 2]");
    assert_eq!(i.draft().attachments(), [f[0].clone()], "文件跟着回来");
    std::fs::remove_dir_all(&dir).unwrap_or_default();
}

#[test]
fn a_kitty_drop_of_a_mixed_batch_splits_by_line_and_keeps_other_files_as_paths() {
    // 2026-09-30 项目主人：kitty 里拖名字带空格的 PDF 进来还是路径；混着一批拖（视频、图、md、txt）全成了路径。
    // kitty 放下几个文件是一行一个、不加引号。
    let (dir, f) = files(
        "kitty",
        &[
            "报告 1.pdf",
            "clip.mp4",
            "grad.png",
            "links.md",
            "my notes.txt",
        ],
    );
    let mut i = attaching();
    i.paste(&f[0].display().to_string());
    assert_eq!(i.editor.text(), "[PDF 1]", "名字带空格的一行是一个文件");
    let mut i = attaching();
    let lines: Vec<String> = f[1..].iter().map(|p| p.display().to_string()).collect();
    i.paste(&lines.join("\n"));
    assert_eq!(
        i.editor.text(),
        "[视频 1] [图片 1] [links.md] [my notes.txt]",
        "认得出的收成附件，别的收成文件块"
    );
    assert_eq!(i.draft().attachments(), [f[1].clone(), f[2].clone()]);
    assert_eq!(
        i.draft().expand(),
        format!("[视频 1] [图片 1] {} '{}'", f[3].display(), f[4].display()),
        "文件块发出去换回路径，带空格的加引号"
    );
    std::fs::remove_dir_all(&dir).unwrap_or_default();
}

#[test]
fn clicking_an_attachment_in_the_box_opens_it_and_leaves_the_cursor() {
    // 2026-09-30 项目主人：输入框里还没发出去的块也能点开。点一下打开文件，光标不动；从块上拖是拖选。
    use ratatui::crossterm::event::{MouseButton, MouseEventKind};

    use super::mouse;
    use crate::input::Action;

    let (dir, f) = files("click", &["a.png"]);
    let mut i = attaching();
    i.editor.insert("看");
    i.paste(&f[0].display().to_string());
    i.editor.insert("吧");
    let end = i.editor.cursor();
    // 框在第 10 列、第 5 行起；「看」占两列，块占第 2 到 10 列。
    let at = |i: &mut InputBox, kind, col: u16| i.mouse(mouse(kind, 10 + col, 5), true);
    let down = MouseEventKind::Down(MouseButton::Left);
    let up = MouseEventKind::Up(MouseButton::Left);
    at(&mut i, MouseEventKind::Moved, 5);
    assert_eq!(i.hovered_attachment(), Some((3, 13)), "悬停在块上");
    at(&mut i, MouseEventKind::Moved, 11);
    assert_eq!(i.hovered_attachment(), None);
    assert_eq!(at(&mut i, down, 9), Action::None);
    assert_eq!(at(&mut i, up, 9), Action::Open(f[0].display().to_string()));
    assert_eq!(i.editor.cursor(), end, "光标不动");
    assert_eq!(i.editor.selection(), None);
    // 从块上按下去拖：拖选，不打开。
    at(&mut i, down, 4);
    i.mouse(
        mouse(MouseEventKind::Drag(MouseButton::Left), 10 + 11, 5),
        true,
    );
    assert_eq!(at(&mut i, up, 11), Action::None);
    assert!(i.editor.selection().is_some(), "选中了");
    // 点块旁边的字：照旧放光标。
    at(&mut i, down, 0);
    at(&mut i, up, 0);
    assert_eq!(i.editor.cursor(), 0);
    std::fs::remove_dir_all(&dir).unwrap_or_default();
}

#[test]
fn a_picked_mention_becomes_a_block_with_a_space_after() {
    // 2026-09-30 项目主人定：@ 列表里选中的和拖进来的一样收成块（「`@` 文件列表」第 5 条）。
    let (dir, f) = files("mention", &["a.png", "notes.txt"]);
    let mut i = attaching();
    i.editor.insert("看 @no");
    i.take_mention("看 ".len(), f[1].clone());
    assert_eq!(i.editor.text(), "看 [notes.txt] ");
    assert_eq!(
        i.draft().expand(),
        format!("看 {} ", f[1].display()),
        "发出去是路径"
    );
    i.editor.insert("@a.");
    let start = i.editor.text().len() - "@a.".len();
    i.take_mention(start, f[0].clone());
    assert_eq!(i.editor.text(), "看 [notes.txt] [图片 1] ", "图片是附件块");
    // Tab 进目录：那个词换成目录，接着列。
    let mut i = attaching();
    i.editor.insert("@sr");
    i.retype_mention(0, "@src/");
    assert_eq!(i.editor.text(), "@src/");
    std::fs::remove_dir_all(&dir).unwrap_or_default();
}
