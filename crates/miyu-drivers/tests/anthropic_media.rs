//! Anthropic 的图片、文件（`docs/blueprint/drivers/anthropic.md`「怎么走：编码」第 4、6、9 条，施工 8-12）：能收的写成
//! `image`、`document` 块，媒体类型不对的、不能收的换成字；工具结果里的图、PDF 放在 `tool_result` 里面；文本文件、带名字的
//! 图片、替它看的图照 openai-chat 一样换成字（`media.rs` 共用）；缺 blob 报错。

use std::collections::{BTreeMap, BTreeSet};

use crate::support::{
    anthropic_sample, claude, file, id, image, named_image, read_tool, sees_all, text, texts,
    tool_call,
};
use miyu_drivers::anthropic::{blobs_needed, encode};
use miyu_drivers::{EncodeError, Inputs};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const BMP: &[u8] = b"BM\0\0\0\0bitmap";
const PDF: &[u8] = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n";
const ZIP: &[u8] = b"PK\x03\x04\x14\x00";
const NOTES: &[u8] = b"# Notes\nline two\n";

/// 晚霞那张的转述。
const SUNSET: &str = "An orange sunset over the sea.";

fn request(messages: Vec<Message>) -> Request {
    Request {
        tools: vec![read_tool()],
        system: "You are Miyu.".to_string(),
        messages,
        stable: 0,
        continuation: false,
        described: Default::default(),
    }
}

fn blobs() -> BTreeMap<ContentHash, Vec<u8>> {
    [PNG, BMP, PDF, ZIP, NOTES]
        .into_iter()
        .map(|content| (ContentHash::of(content), content.to_vec()))
        .collect()
}

fn body(request: &Request, inputs: Inputs) -> Vec<u8> {
    encode(request, &claude(inputs, None), &texts(), &blobs())
        .expect("要用的 blob 都在")
        .body
}

/// 线上第 `n` 条消息的 `content`，去掉打点。
fn content(body: &[u8], n: usize) -> Value {
    let text = String::from_utf8(body.to_vec()).expect("UTF-8");
    let value: Value =
        serde_json::from_str(&text.replace(r#","cache_control":{"type":"ephemeral"}"#, ""))
            .expect("是 JSON");
    value["messages"][n]["content"].clone()
}

/// 一条 user：字、PNG、BMP、PDF、压缩包。
fn attachments() -> Request {
    request(vec![Message::User {
        blocks: vec![
            text("看看这些"),
            image(PNG, "image/png"),
            image(BMP, "image/bmp"),
            file(PDF, "报告.pdf", "application/pdf"),
            file(ZIP, "data.zip", "application/zip"),
        ],
    }])
}

fn base64(bytes: &[u8]) -> String {
    miyu_drivers::base64::encode(bytes)
}

#[test]
fn images_and_pdfs_go_as_blocks_and_the_rest_as_placeholders() {
    let body = body(&attachments(), sees_all());
    anthropic_sample("media", &body);
    let said = content(&body, 0);
    assert_eq!(said[0], json!({"type": "text", "text": "看看这些"}));
    assert_eq!(
        said[1],
        json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": base64(PNG)}})
    );
    // BMP 这一家不收：换成占位。
    assert_eq!(
        said[2],
        json!({"type": "text", "text": texts().image_omitted(None)})
    );
    assert_eq!(
        said[3],
        json!({
            "type": "document",
            "source": {"type": "base64", "media_type": "application/pdf", "data": base64(PDF)},
            "title": "报告.pdf"
        })
    );
    assert_eq!(
        said[4],
        json!({"type": "text", "text": texts().file_omitted("data.zip", "application/zip", ZIP.len())})
    );
}

#[test]
fn a_model_that_takes_neither_gets_placeholders() {
    let body = body(&attachments(), Inputs::default());
    anthropic_sample("media-omitted", &body);
    let said = content(&body, 0);
    assert_eq!(said[1]["text"], texts().image_omitted(None));
    assert_eq!(
        said[3]["text"],
        texts().file_omitted("报告.pdf", "application/pdf", PDF.len())
    );
}

#[test]
fn pictures_and_pdfs_in_a_tool_result_stay_inside_it() {
    let request = request(vec![
        Message::User {
            blocks: vec![text("读图和报告")],
        },
        Message::Assistant {
            blocks: vec![
                tool_call("call_1_1", "read", "{}", None),
                tool_call("call_1_2", "read", "{}", None),
            ],
        },
        Message::Tool {
            call_id: id("call_1_1"),
            error: false,
            blocks: vec![image(PNG, "image/png")],
        },
        Message::Tool {
            call_id: id("call_1_2"),
            error: false,
            blocks: vec![
                text("PDF, 1 page"),
                file(PDF, "报告.pdf", "application/pdf"),
            ],
        },
    ]);
    let body = body(&request, sees_all());
    anthropic_sample("tool-media", &body);
    let results = content(&body, 2);
    assert_eq!(results.as_array().map(Vec::len), Some(2));
    assert_eq!(results[0]["content"][0]["type"], "image");
    assert_eq!(results[0]["content"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        results[1]["content"][0],
        json!({"type": "text", "text": "PDF, 1 page"})
    );
    assert_eq!(results[1]["content"][1]["type"], "document");
    let text = String::from_utf8(body).expect("UTF-8");
    assert!(!text.contains(&texts().tool_attachments()));
    assert!(!text.contains(&texts().tool_attachments_only()));
}

#[test]
fn text_files_go_in_as_text() {
    let request = request(vec![Message::User {
        blocks: vec![text("看看"), file(NOTES, "notes.md", "text/markdown")],
    }]);
    let body = body(&request, sees_all());
    anthropic_sample("text-files", &body);
    assert_eq!(
        content(&body, 0)[1],
        json!({"type": "text", "text": texts().text_file("notes.md", "# Notes\nline two\n").expect("有那三句")})
    );
}

#[test]
fn named_images_are_wrapped_in_tags_or_named_in_the_placeholder() {
    let request = request(vec![Message::User {
        blocks: vec![named_image(PNG, "晚霞.png", "image/png")],
    }]);
    let body = body(&request, sees_all());
    anthropic_sample("image-names", &body);
    let (open, close) = texts().image_tags(Some("晚霞.png")).expect("有标签");
    let said = content(&body, 0);
    assert_eq!(said[0], json!({"type": "text", "text": open}));
    assert_eq!(said[1]["type"], "image");
    assert_eq!(said[2], json!({"type": "text", "text": close}));
    let blind = content(&self::body(&request, Inputs::default()), 0);
    assert_eq!(
        blind,
        json!([{"type": "text", "text": texts().image_omitted(Some("晚霞.png"))}])
    );
}

#[test]
fn a_blind_model_reads_the_description_in_the_pictures_place() {
    let mut request = request(vec![Message::User {
        blocks: vec![
            text("这是什么？"),
            named_image(PNG, "晚霞.png", "image/png"),
        ],
    }]);
    request
        .described
        .insert(ContentHash::of(PNG), SUNSET.to_string());
    let blind = body(&request, Inputs::default());
    anthropic_sample("image-descriptions", &blind);
    assert_eq!(
        content(&blind, 0)[1],
        json!({"type": "text", "text": texts().image_described(Some("晚霞.png"), SUNSET).expect("有标签")})
    );
    // 看得了图的不看转述。
    assert_eq!(content(&body(&request, sees_all()), 0)[2]["type"], "image");
}

#[test]
fn the_blobs_it_needs_and_a_missing_one_is_reported() {
    let request = attachments();
    let needed = blobs_needed(&request, &claude(sees_all(), None));
    let want: BTreeSet<ContentHash> = [PNG, BMP, PDF, ZIP]
        .into_iter()
        .map(ContentHash::of)
        .collect();
    assert_eq!(needed, want);
    let blind = blobs_needed(&request, &claude(Inputs::default(), None));
    let files: BTreeSet<ContentHash> = [PDF, ZIP].into_iter().map(ContentHash::of).collect();
    assert_eq!(blind, files);
    let error = encode(
        &request,
        &claude(sees_all(), None),
        &texts(),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(error, EncodeError::MissingBlob(ContentHash::of(PNG)));
}

#[test]
fn an_image_block_is_not_written_into_an_assistant_reply() {
    let request = request(vec![
        Message::User {
            blocks: vec![text("画")],
        },
        Message::Assistant {
            blocks: vec![image(PNG, "image/png"), text("画好了")],
        },
        Message::User {
            blocks: vec![text("好")],
        },
    ]);
    assert_eq!(
        content(&body(&request, sees_all()), 1),
        json!([{"type": "text", "text": "画好了"}])
    );
}
