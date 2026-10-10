//! 取东西的测试共用的（施工 O-33，`tests/media.rs`）：一张量得出宽高的小 PNG，NapCat 段的样子（图、表情、视频、文件、语音），
//! `get_msg` 回的消息，工具结果整块读出来，她某一次请求里人这边的图片块，blob 在不在某个账号名下。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::id::{AccountId, ContentHash};
use miyu_kernel::request::{Message, Request};
use miyu_store::blob::Blobs;

use super::Home;
use super::group::*;
use super::platform::{GROUP, venue};

/// 一张 `width` × `height` 的 PNG 的开头：签名和 IHDR，核心量宽高只看它（同核心的测试）。
pub fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 内容的 base64。
pub fn encoded(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// 一段图：`file` 是 NapCat 给的编号（文件名），大小 `size`。
pub fn image(file: &str, size: u64) -> Value {
    json!({"type": "image", "data": {"file": file, "sub_type": 0, "summary": "", "url": "https://example.invalid/x", "file_size": size.to_string()}})
}

/// 一段小黄脸。
pub fn face(id: &str, text: &str) -> Value {
    json!({"type": "face", "data": {"id": id, "raw": {"faceText": text}}})
}

/// 一段文件：名字 `name`，编号 `id`，大小 `size`。
pub fn file(name: &str, id: &str, size: u64) -> Value {
    json!({"type": "file", "data": {"file": name, "file_id": id, "file_size": size.to_string()}})
}

/// 一段视频：编号 `id`，大小 `size`。
pub fn video(id: &str, size: u64) -> Value {
    json!({"type": "video", "data": {"file": id, "file_size": size.to_string()}})
}

/// 一段语音：编号 `id`，大小 `size`。
pub fn voice(id: &str, size: u64) -> Value {
    json!({"type": "record", "data": {"file": id, "file_size": size.to_string()}})
}

/// `get_msg` 回的、这个群里的一条（段是 `message`）。
pub fn in_group(message: Value) -> Value {
    json!({"message_type": "group", "group_id": GROUP, "user_id": 40001, "message": message})
}

/// 这个群的第 `turn` 轮完了以后，工具结果的 `body`（整块：图片块也在里面），照先后。
pub async fn results(home: &Home, turn: usize) -> Vec<Value> {
    let events = until_events(&home.root, &venue(), |events| {
        of_kind(events, "turn.ended").len() >= turn
    })
    .await;
    of_kind(&events, "tool.result")
        .into_iter()
        .map(|result| result["body"].clone())
        .collect()
}

/// 工具结果头一块的字，去掉行尾。
pub fn text_of(result: &Value) -> String {
    result["blocks"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .trim_end()
        .to_string()
}

/// 一次请求里人这边的图片块：名字和 blob，照先后。
pub fn pictures(request: &Request) -> Vec<(String, ContentHash)> {
    request
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::User { blocks } => Some(blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::Image(image) => Some((
                image
                    .name
                    .as_ref()
                    .map(|name| name.as_str().to_string())
                    .unwrap_or_default(),
                image.blob.clone(),
            )),
            _ => None,
        })
        .collect()
}

/// 一次请求里人这边的字，接起来。
pub fn user_text(request: &Request) -> String {
    request
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::User { blocks } => Some(blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 账号 `account` 名下有没有内容是 `bytes` 的 blob。
pub fn has_blob(home: &Home, account: &str, bytes: &[u8]) -> bool {
    let account = AccountId::parse(account).expect("合写法");
    Blobs::new(home.root.blobs(&account))
        .get(&ContentHash::of(bytes))
        .is_ok_and(|kept| kept == bytes)
}
