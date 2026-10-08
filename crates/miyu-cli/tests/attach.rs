//! `miyu ask --file`（施工 3-9 三补，`docs/blueprint/cli/ask.md`「附件」）：在进程里起一个核心，在真的套接字上走一遍：
//! 附件照写的先后传上去、跟着这一句发出去；传不上的说是哪个文件、为什么，不发话、不留下空的会话，退出码 1。

use std::sync::Arc;

use crate::support::outside::Outside;
use crate::support::{Asked, Home, plan};
use miyu_cli::Plan;
use miyu_kernel::block::Block;
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};

/// 一张 2 × 3 的 PNG 的开头：签名和 IHDR，量宽高只看它。
fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&2u32.to_be_bytes());
    bytes.extend_from_slice(&3u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

#[tokio::test]
async fn files_go_with_the_words_in_the_order_given() {
    let home = Home::new(Arc::new(Script::new([Play::Says("看到了。")])));
    let outside = Outside::new();
    let notes = outside.file("notes.md", "# 待办\n");
    let shot = outside.0.join("shot.png");
    std::fs::write(&shot, png()).unwrap();
    let with = Plan {
        files: vec![
            notes.to_string_lossy().into_owned(),
            shot.to_string_lossy().into_owned(),
        ],
        ..plan("看看这两个")
    };
    let Asked { code, out, err, .. } = home.ask(&with).await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, "看到了。\n");
    let session = home.sessions()[0].clone();
    let said: Vec<Vec<Block>> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MessageUser(message) => Some(message.blocks),
            _ => None,
        })
        .collect();
    let [blocks] = said.as_slice() else {
        panic!("说了一句：{said:?}");
    };
    match blocks.as_slice() {
        [Block::Text(text), Block::File(file), Block::Image(image)] => {
            assert_eq!(text.text, "看看这两个");
            assert_eq!(file.name.as_str(), "notes.md");
            assert_eq!(file.media_type.as_str(), "text/plain");
            assert_eq!((image.width, image.height), (2, 3));
        }
        other => panic!("文字在前，附件照写的先后：{other:?}"),
    }
}

#[tokio::test]
async fn a_file_that_cannot_go_is_named_and_nothing_is_said() {
    let home = Home::new(Arc::new(Script::new([])));
    let outside = Outside::new();
    let fine = outside.file("fine.md", "好的\n");
    let missing = outside.0.join("missing.txt").to_string_lossy().into_owned();
    let with = Plan {
        files: vec![fine.to_string_lossy().into_owned(), missing.clone()],
        ..plan("看看")
    };
    let Asked { code, out, err, .. } = home.ask(&with).await;
    assert_eq!(code, 1, "{err}");
    assert_eq!(out, "");
    assert_eq!(
        err,
        format!("附不上 {missing}：读不了这个文件：没有、不是普通文件，或者没有权限。\n")
    );
    assert!(home.sessions().is_empty(), "没造会话");
    // 太大的一样：说是哪个、为什么。
    let big = outside.0.join("big.bin");
    std::fs::write(&big, vec![0u8; 20 * 1024 * 1024 + 1]).unwrap();
    let big = big.to_string_lossy().into_owned();
    let with = Plan {
        files: vec![big.clone()],
        language: miyu_cli::language::Language::English,
        ..plan("look")
    };
    let Asked { code, err, .. } = home.ask(&with).await;
    assert_eq!(code, 1, "{err}");
    assert!(
        err.starts_with(&format!("Cannot attach {big}: The attachment is too big")),
        "{err}"
    );
    assert!(home.sessions().is_empty(), "没造会话");
}
