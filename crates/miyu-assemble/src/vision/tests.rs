//! 转述一张图的请求（施工 8-17）：一条 user，不带 system、工具面；第一块是指令，有人的话的接那一行和原话（原样、不转义），
//! 第二块是这张图、去掉了名字；快照里没有转述的字的，组装器交回没有。

use miyu_kernel::assemble::Assembler;
use miyu_kernel::block::{Block, Image};
use miyu_kernel::id::{ContentHash, FileName, MediaType};
use miyu_kernel::request::Message;

use super::*;
use crate::test_support::{texts, vision_texts};
use crate::{DefaultAssembler, Stable, Texts};

/// 一张带名字的图。
fn picture() -> Image {
    Image {
        blob: ContentHash::of(b"png"),
        name: Some(FileName::parse("晚霞.png").expect("名字合写法")),
        media_type: MediaType::parse("image/png").expect("媒体类型合写法"),
        width: 640,
        height: 480,
        path: None,
    }
}

/// 照 `texts` 造的组装器，稳定区里有工具、system：转述的请求都不带。
fn assembler(texts: Texts) -> DefaultAssembler {
    let stable = Stable {
        tools: Vec::new(),
        system: "You are Miyu.".to_string(),
        demos: Vec::new(),
    };
    DefaultAssembler::new(stable, texts)
}

/// 转述请求那一条 user 的两块：字、图。
fn blocks(request: &Request) -> (String, Image) {
    assert!(request.tools.is_empty());
    assert_eq!(request.system, "");
    assert_eq!((request.stable, request.continuation), (0, false));
    assert!(request.described.is_empty());
    let [Message::User { blocks }] = request.messages.as_slice() else {
        panic!("只有一条 user：{:?}", request.messages);
    };
    let [Block::Text(text), Block::Image(image)] = blocks.as_slice() else {
        panic!("先字后图：{blocks:?}");
    };
    (text.text.clone(), image.clone())
}

#[test]
fn the_request_is_the_instruction_then_the_picture_without_its_name() {
    let request = assembler(texts())
        .describe(&picture(), None)
        .expect("快照里有转述的字");
    let (text, image) = blocks(&request);
    assert_eq!(text, "<describe>\n", "没有人的话的只有指令");
    assert_eq!(
        image,
        Image {
            name: None,
            ..picture()
        },
        "图照原样，只去掉名字"
    );
}

/// 人的话接在那一行后面，原样：尖括号、引号、换行都不转义，也不截。
#[test]
fn the_latest_words_follow_the_question_as_they_are() {
    let said = "图里第三行写的是什么？\n<b>\"加粗\"</b> 那个".repeat(50);
    let request = assembler(texts())
        .describe(&picture(), Some(&said))
        .expect("快照里有转述的字");
    let (text, _) = blocks(&request);
    let Vision {
        instruction,
        question,
    } = vision_texts();
    assert_eq!(text, format!("{instruction}{question}{said}"));
}

#[test]
fn an_old_snapshot_does_not_describe() {
    let old = Texts {
        vision: None,
        ..texts()
    };
    assert_eq!(assembler(old).describe(&picture(), Some("看看")), None);
}
