//! 说话带附件（施工 3-9 三补，`docs/blueprint/protocol.md` 的 `session.send`）：`blob.put` 的回应照先后变成
//! `message.user` 里文字后面的图片块、文件块，宽高、媒体类型照核心自己量的；只有附件也是一句话；blob 不在的拒绝，
//! 什么都不写。

mod support;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, File, Image, Text};
use miyu_kernel::event::Body;
use miyu_kernel::id::{ContentHash, FileName, MediaType};
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};

use support::*;

/// 一张 `width` × `height` 的 PNG 的开头。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 传一个文件，交回 `blob.put` 的回应。
async fn put(client: &mut Client, home: &Home, name: &str, bytes: &[u8]) -> Value {
    let path = home.work.join(name);
    std::fs::write(&path, bytes).expect("写得了");
    let reply = client.call("p", "blob.put", json!({"path": path})).await;
    reply["result"].clone()
}

/// 会话日志里人说的话，照先后。
fn said(home: &Home, session: &str) -> Vec<Vec<Block>> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MessageUser(message) => Some(message.blocks),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn attachments_follow_the_text_as_measured_by_the_core() {
    let home = Home::new();
    let script = Script::new([Play::Says("看到了。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let picture = png(800, 600);
    let image = put(&mut client, &home, "shot.png", &picture).await;
    let notes = put(&mut client, &home, "notes.md", "# 待办\n".as_bytes()).await;
    // 头交回来的宽高、种类是错的：不算，照核心自己量的。
    let mut lying = image.clone();
    lying["width"] = json!(1);
    lying["kind"] = json!("file");
    let session = client.create("c1", "~").await;
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "看看", "attachments": [lying, notes]}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 1).await;
    let expected = vec![
        Block::Text(Text {
            text: "看看".to_string(),
        }),
        Block::Image(Image {
            blob: ContentHash::of(&picture),
            name: Some(FileName::parse("shot.png").unwrap()),
            media_type: MediaType::parse("image/png").unwrap(),
            width: 800,
            height: 600,
        }),
        Block::File(File {
            blob: ContentHash::of("# 待办\n".as_bytes()),
            name: FileName::parse("notes.md").unwrap(),
            media_type: MediaType::parse("text/plain").unwrap(),
        }),
    ];
    assert_eq!(said(&home, &session), std::slice::from_ref(&expected));
    // 她看到的请求里，最后一条人说的就是这几块。
    let requests = script.requests();
    let Some(Message::User { blocks }) = requests[0].1.messages.last() else {
        panic!("最后一条是人说的");
    };
    assert!(blocks.ends_with(&expected), "{blocks:?}");
}

#[tokio::test]
async fn attachments_alone_are_a_message() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let pdf = put(&mut client, &home, "a.pdf", b"%PDF-1.7\n").await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "", "attachments": [pdf]}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    let blocks = said(&home, &session);
    assert!(
        matches!(blocks.as_slice(), [only] if matches!(only.as_slice(), [Block::File(file)] if file.media_type.as_str() == "application/pdf")),
        "{blocks:?}"
    );
    // 没有字、也没有附件的，照旧是空消息；`null` 是没有附件。
    let reply = client
        .call(
            "c3",
            "session.send",
            json!({"session": session, "text": "", "attachments": null}),
        )
        .await;
    assert_eq!(reason(&reply), Some("empty_message"), "{reply}");
}

#[tokio::test]
async fn a_blob_the_core_does_not_have_is_refused_and_nothing_is_written() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &work).await;
    let before = home.log(&session).len();
    let missing = json!({"blob": ContentHash::of(b"never put").as_str(), "name": "a.txt", "media_type": "text/plain"});
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "看看", "cwd": "/elsewhere", "attachments": [missing]}),
        )
        .await;
    assert_eq!(reason(&reply), Some("unknown_attachment"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        "附件不在核心里：先用 blob.put 传上来。"
    );
    assert_eq!(home.log(&session).len(), before, "什么都没写");
    // 附件先查、再找会话：换过的工作目录也没送进会话，下一句开的回合照旧。
    let reply = client.say("c3", &session, "在吗").await;
    assert_eq!(reply["result"]["cwd"], work.as_str(), "{reply}");
}

#[tokio::test]
async fn attachments_that_do_not_fit_are_bad_params() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let blob = ContentHash::of(b"x");
    for attachment in [
        json!({"name": "a.txt", "media_type": "text/plain"}),
        json!({"blob": "sha256:nothex", "name": "a.txt", "media_type": "text/plain"}),
        json!({"blob": blob.as_str(), "media_type": "text/plain"}),
        json!({"blob": blob.as_str(), "name": "a/b", "media_type": "text/plain"}),
        json!({"blob": blob.as_str(), "name": "a.txt"}),
        json!({"blob": blob.as_str(), "name": "a.txt", "media_type": "TEXT"}),
        json!("sha256:…"),
    ] {
        let reply = client
            .call(
                "c2",
                "session.send",
                json!({"session": session, "text": "x", "attachments": [attachment]}),
            )
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{attachment}：{reply}");
    }
    let reply = client
        .call(
            "c3",
            "session.send",
            json!({"session": session, "text": "x", "attachments": {}}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(home.log(&session).len(), 1, "什么都没写");
}
