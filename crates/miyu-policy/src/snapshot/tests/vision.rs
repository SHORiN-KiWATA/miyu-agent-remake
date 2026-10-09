//! 替看图进快照（施工 8-17，`docs/blueprint/policy.md`「`CoreTexts`」）：出厂的快照带着转述请求的两份字和图的位置的三句标签，
//! 造出的组装器交得出转述的请求、驱动写得出带标签的转述；以前造的快照没有这两格，读进来再写出去一字不差，不转述、照旧写
//! 占位。

use std::collections::BTreeMap;

use miyu_drivers::openai_chat::{Compat, encode};
use miyu_drivers::{Call, Inputs};
use miyu_kernel::block::{Block, Image, Text};
use miyu_kernel::id::{ContentHash, FileName, MediaType, ModelName};
use miyu_kernel::request::{Message, Request};

use super::*;

/// 一张带名字的图。
fn picture() -> Image {
    Image {
        blob: ContentHash::of(b"png"),
        name: Some(FileName::parse("晚霞.png").unwrap()),
        media_type: MediaType::parse("image/png").unwrap(),
        width: 640,
        height: 480,
        path: None,
    }
}

/// 照快照造的组装器交出的转述请求，那一条 user 的字。
fn asked(snapshot: &Snapshot) -> Option<String> {
    let request = snapshot
        .policy()
        .unwrap()
        .assembler
        .describe(&picture(), Some("哪张是晚霞？"))?;
    match &request.messages[..] {
        [Message::User { blocks }] => match &blocks[..] {
            [Block::Text(text), Block::Image(_)] => Some(text.text.clone()),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

/// 照快照的驱动占位，给看不了图的模型编码一条带图、图有转述的 user，交回它的 `content`。
fn written(snapshot: &Snapshot) -> String {
    let request = Request {
        tools: Vec::new(),
        system: String::new(),
        messages: vec![Message::User {
            blocks: vec![
                Block::Text(Text {
                    text: "看".to_string(),
                }),
                Block::Image(picture()),
            ],
        }],
        stable: 0,
        continuation: false,
        described: [(picture().blob, "Sunset.".to_string())].into(),
    };
    let call = Call {
        model: ModelName::parse("m").unwrap(),
        max_output: None,
        inputs: Inputs::default(),
        effort: None,
        temperature: None,
    };
    let texts = snapshot.driver_texts().unwrap();
    let body = encode(
        &request,
        &call,
        &Compat::default(),
        &texts,
        &BTreeMap::new(),
    )
    .unwrap()
    .body;
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    body["messages"][0]["content"].as_str().unwrap().to_string()
}

#[test]
fn vision_texts_go_in_and_older_snapshots_lack_them() {
    let snapshot = engineer();
    let text = asked(&snapshot).expect("出厂的交得出转述的请求");
    assert!(
        text.starts_with("Describe what this image shows")
            && text.ends_with("so you know what matters most:\n哪张是晚霞？"),
        "{text}"
    );
    assert_eq!(
        written(&snapshot),
        "看\n<image-description name=\"晚霞.png\">\nSunset.\n</image-description>\n"
    );

    let bytes = String::from_utf8(snapshot.to_bytes()).unwrap();
    let cut = |bytes: &str, from: &str, to: &str| {
        let start = bytes.find(from).unwrap();
        let end = start + bytes[start..].find(to).unwrap() + to.len();
        bytes[..start].to_string() + &bytes[end..]
    };
    let older = cut(&bytes, r#","image_description":{"#, r#"\n"}"#);
    let older = cut(&older, r#","vision":{"instruction""#, r#"\n"}"#);
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert!(read.core.vision.is_none() && read.core.drivers.image_description.is_none());
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    assert_eq!(asked(&read), None, "不转述");
    assert_eq!(
        written(&read),
        "看\nAn image was attached here (晚霞.png), but this model cannot view images.\n",
        "照旧写占位"
    );
}
