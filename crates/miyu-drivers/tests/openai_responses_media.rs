//! Responses 的图片、文件（`docs/blueprint/drivers/openai-responses.md`「编码」第 3、5 条，施工 8-13）：能收的写成
//! `input_image`、`input_file`，不能收的换成字；工具结果里的图、PDF 放在 `output` 里；文本文件、带名字的图片、替它看的图照
//! openai-chat 一样换成字（`media.rs` 共用）；缺 blob 报错。

use std::collections::BTreeMap;

use crate::support::{
    file, gpt, id, image, named_image, read_tool, responses_sample, sees_all, text, texts,
    tool_call,
};
use miyu_drivers::openai_responses::{blobs_needed, encode};
use miyu_drivers::{EncodeError, Inputs};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const PDF: &[u8] = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n";
const ZIP: &[u8] = b"PK\x03\x04\x14\x00";
const NOTES: &[u8] = b"# Notes\nline two\n";
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
    [PNG, PDF, ZIP, NOTES]
        .into_iter()
        .map(|content| (ContentHash::of(content), content.to_vec()))
        .collect()
}

fn body(request: &Request, inputs: Inputs) -> Vec<u8> {
    encode(request, &gpt(inputs, None), &texts(), &blobs())
        .expect("要用的 blob 都在")
        .body
}

fn item(body: &[u8], n: usize) -> Value {
    let value: Value = serde_json::from_slice(body).expect("是 JSON");
    value["input"][n].clone()
}

fn url(media_type: &str, bytes: &[u8]) -> String {
    format!(
        "data:{media_type};base64,{}",
        miyu_drivers::base64::encode(bytes)
    )
}

fn attachments() -> Request {
    request(vec![Message::User {
        blocks: vec![
            text("看看这些"),
            image(PNG, "image/png"),
            text("还有"),
            file(PDF, "报告.pdf", "application/pdf"),
            file(ZIP, "data.zip", "application/zip"),
        ],
    }])
}

#[test]
fn images_and_pdfs_go_as_parts_and_the_rest_as_text() {
    let body = body(&attachments(), sees_all());
    responses_sample("media", &body);
    let zip = texts().file_omitted("data.zip", "application/zip", ZIP.len());
    assert_eq!(
        item(&body, 0),
        json!({"role": "user", "content": [
            {"type": "input_text", "text": "看看这些"},
            {"type": "input_image", "image_url": url("image/png", PNG)},
            {"type": "input_text", "text": "还有"},
            {"type": "input_file", "filename": "报告.pdf", "file_data": url("application/pdf", PDF)},
            {"type": "input_text", "text": zip}
        ]})
    );
}

#[test]
fn a_model_that_takes_neither_gets_one_string() {
    let body = body(&attachments(), Inputs::default());
    responses_sample("media-omitted", &body);
    let content = item(&body, 0)["content"].clone();
    let said = content.as_str().expect("全是字的是一个字符串");
    assert!(said.contains(&texts().image_omitted(None)));
    assert!(said.contains(&texts().file_omitted("报告.pdf", "application/pdf", PDF.len())));
}

#[test]
fn pictures_and_pdfs_in_a_tool_result_stay_in_its_output() {
    let request = request(vec![
        Message::User {
            blocks: vec![text("读图")],
        },
        Message::Assistant {
            blocks: vec![tool_call("call_1_1", "read", "{}", None)],
        },
        Message::Tool {
            call_id: id("call_1_1"),
            error: false,
            blocks: vec![
                image(PNG, "image/png"),
                file(PDF, "报告.pdf", "application/pdf"),
            ],
        },
    ]);
    let body = body(&request, sees_all());
    responses_sample("tool-media", &body);
    let output = item(&body, 2);
    assert_eq!(output["type"], "function_call_output");
    assert_eq!(output["output"][0]["type"], "input_image");
    assert_eq!(output["output"][1]["type"], "input_file");
    let value: Value = serde_json::from_slice(&body).expect("JSON");
    assert_eq!(
        value["input"].as_array().map(Vec::len),
        Some(3),
        "不挪到后面"
    );
}

#[test]
fn text_files_named_images_and_descriptions_are_written_like_openai_chat() {
    let mut request = request(vec![Message::User {
        blocks: vec![
            file(NOTES, "notes.md", "text/markdown"),
            named_image(PNG, "晚霞.png", "image/png"),
        ],
    }]);
    let seen = body(&request, sees_all());
    responses_sample("text-files", &seen);
    let notes = texts()
        .text_file("notes.md", "# Notes\nline two\n")
        .expect("有那三句");
    let (open, close) = texts().image_tags(Some("晚霞.png")).expect("有标签");
    assert_eq!(
        item(&seen, 0)["content"],
        json!([
            {"type": "input_text", "text": format!("{notes}{open}")},
            {"type": "input_image", "image_url": url("image/png", PNG)},
            {"type": "input_text", "text": close}
        ])
    );
    responses_sample("image-names", &body(&request, Inputs::default()));
    request
        .described
        .insert(ContentHash::of(PNG), SUNSET.to_string());
    let blind = body(&request, Inputs::default());
    responses_sample("image-descriptions", &blind);
    let described = texts()
        .image_described(Some("晚霞.png"), SUNSET)
        .expect("有标签");
    assert!(
        item(&blind, 0)["content"]
            .as_str()
            .expect("一个字符串")
            .ends_with(&described)
    );
}

#[test]
fn the_blobs_it_needs_and_a_missing_one_is_reported() {
    let request = attachments();
    assert_eq!(blobs_needed(&request, &gpt(sees_all(), None)).len(), 3);
    assert_eq!(
        blobs_needed(&request, &gpt(Inputs::default(), None)).len(),
        2
    );
    let error = encode(&request, &gpt(sees_all(), None), &texts(), &BTreeMap::new()).unwrap_err();
    assert_eq!(error, EncodeError::MissingBlob(ContentHash::of(PNG)));
}
