//! 照模型收（蓝图 `tui.md`「输入框」第 12 条「照模型收」）：这个模型收不了的那一种，拖进来、贴进来时收成文件块（发出去写
//! 路径），记下改了哪几种好提示一句；附了以后才换的模型，发的那一刻照它再改一遍。

use std::path::PathBuf;

use super::attach_rule;
use super::paste::{folding, submit};
use crate::input::InputBox;

fn file(tag: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("miyu-refused-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, b"x").unwrap();
    p
}

fn attaching(refused: &[&str]) -> InputBox {
    let mut i = folding();
    i.set_attach_rule(attach_rule());
    i.set_refused(refused.iter().map(|k| (*k).to_string()).collect());
    i
}

#[test]
fn a_kind_the_model_cannot_take_becomes_a_path_when_dropped_or_pasted() {
    let png = file("drop", "a.png");
    let pdf = file("drop", "b.pdf");
    let mut i = attaching(&["image"]);
    i.paste_files(&format!("{}\n{}", png.display(), pdf.display()));
    assert_eq!(i.take_demoted(), ["image"], "改了哪几种，提示一句");
    assert!(i.take_demoted().is_empty(), "提示过的不再提");
    let draft = submit(&mut i);
    assert_eq!(
        draft.attachments(),
        std::slice::from_ref(&pdf),
        "收得了的照旧当附件"
    );
    assert!(
        draft.expand().contains(&png.display().to_string()),
        "收不了的写路径"
    );
    // 剪贴板贴的图（`attach`）也一样。
    let mut i = attaching(&["image"]);
    i.attach(png.clone(), "image");
    assert_eq!(i.take_demoted(), ["image"]);
    assert!(submit(&mut i).attachments().is_empty());
    // 都收得了的照旧。
    let mut i = attaching(&[]);
    i.attach(png.clone(), "image");
    assert!(i.take_demoted().is_empty());
    assert_eq!(submit(&mut i).attachments(), [png]);
}

#[test]
fn attachments_made_before_switching_models_are_demoted_when_sent() {
    let png = file("send", "c.png");
    let mut i = attaching(&[]);
    i.attach(png.clone(), "image");
    let mut draft = submit(&mut i);
    assert_eq!(draft.demote(&["image".to_string()]), ["image"]);
    assert!(draft.attachments().is_empty());
    assert!(draft.expand().contains(&png.display().to_string()));
    let mut kept = {
        let mut i = attaching(&[]);
        i.attach(png.clone(), "image");
        submit(&mut i)
    };
    assert!(kept.demote(&["pdf".to_string()]).is_empty());
    assert_eq!(kept.attachments(), [png]);
}
