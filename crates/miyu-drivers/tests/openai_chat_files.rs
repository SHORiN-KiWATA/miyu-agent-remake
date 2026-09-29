//! 人附的文件怎么进请求（施工 3-9 三补，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：文本文件照字放进消息，带着
//! 文件名，最多 64 KiB，截过的写明；二进制的、发不了的 PDF 写一句占位，带大小；以前造的快照里没有那三句的，文本文件
//! 也写占位。

mod support;

use std::collections::BTreeMap;

use miyu_drivers::openai_chat::{Compat, EncodeError, encode};
use miyu_drivers::{DriverTextSources, DriverTexts, Inputs};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};
use serde_json::Value;
use support::{call, file, id, read_tool, sample, text, texts, tool_call};

const NOTES: &[u8] = "# 待办\n- 写测试\n- 跑 CI".as_bytes();
const EMPTY: &[u8] = b"";
const BINARY: &[u8] = b"\x00\x01\x02 not text";
const LATIN1: &[u8] = b"caf\xE9\n";
const PDF: &[u8] = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n";

fn request(messages: Vec<Message>) -> Request {
    Request {
        tools: vec![read_tool()],
        system: "You are a helpful assistant.".to_string(),
        messages,
        stable: 0,
        continuation: false,
    }
}

fn blobs(contents: &[&[u8]]) -> BTreeMap<ContentHash, Vec<u8>> {
    contents
        .iter()
        .map(|content| (ContentHash::of(content), content.to_vec()))
        .collect()
}

/// 一条 user 消息：一句话，一份 Markdown、一份空的、一份二进制的、一份 Latin-1 的、一份 PDF。
fn attached() -> Request {
    request(vec![Message::User {
        blocks: vec![
            text("看看这几个文件"),
            file(NOTES, "notes.md", "text/markdown"),
            file(EMPTY, "empty.txt", "text/plain"),
            file(BINARY, "data.bin", "application/octet-stream"),
            file(LATIN1, "old.txt", "text/plain"),
            file(PDF, "报告.pdf", "application/pdf"),
        ],
    }])
}

fn encoded(request: &Request, inputs: Inputs, texts: &DriverTexts) -> Result<Vec<u8>, EncodeError> {
    let blobs = blobs(&[NOTES, EMPTY, BINARY, LATIN1, PDF]);
    encode(
        request,
        &call(inputs, None),
        &Compat::default(),
        texts,
        &blobs,
    )
    .map(|e| e.body)
}

/// 线上第 `n` 条消息的 `content`。
fn content(body: &[u8], n: usize) -> Value {
    let body: Value = serde_json::from_slice(body).expect("是 JSON");
    body["messages"][n]["content"].clone()
}

#[test]
fn text_files_go_in_as_text_and_the_rest_as_placeholders() {
    // 全是文字，拼成一个字符串：文本文件照字，带着文件名；二进制的、不是 UTF-8 的、读不了的 PDF 写占位，带大小。
    let body = encoded(&attached(), Inputs::default(), &texts()).expect("blob 都在");
    sample("text-files", &body);
    let said = content(&body, 1);
    let said = said.as_str().expect("一个字符串");
    assert!(
        said.contains("<file name=\"notes.md\">\n# 待办\n- 写测试\n- 跑 CI\n</file>\n"),
        "{said}"
    );
    assert!(
        said.contains("<file name=\"empty.txt\">\n</file>\n"),
        "{said}"
    );
    assert!(
        said.contains("(data.bin, application/octet-stream, 12 bytes)"),
        "{said}"
    );
    assert!(said.contains("(old.txt, text/plain, 5 bytes)"), "{said}");
    assert!(
        said.contains("(报告.pdf, application/pdf, 15 bytes)"),
        "{said}"
    );
}

#[test]
fn a_pdf_the_model_can_read_still_goes_as_a_file() {
    let inputs = Inputs {
        images: false,
        pdf: true,
    };
    let body = encoded(&attached(), inputs, &texts()).expect("blob 都在");
    let parts = content(&body, 1);
    let last = parts
        .as_array()
        .and_then(|parts| parts.last())
        .expect("分成几段");
    assert_eq!(last["type"], "file");
    assert_eq!(last["file"]["filename"], "报告.pdf");
}

#[test]
fn a_long_text_file_is_cut_to_64_kib_and_says_so() {
    let long = format!("{}\n尾巴", "a".repeat(70_000));
    let request = request(vec![Message::User {
        blocks: vec![file(long.as_bytes(), "big.log", "text/plain")],
    }]);
    let blobs = blobs(&[long.as_bytes()]);
    let body = encode(
        &request,
        &call(Inputs::default(), None),
        &Compat::default(),
        &texts(),
        &blobs,
    )
    .expect("blob 在")
    .body;
    let said = content(&body, 1);
    let said = said.as_str().expect("一个字符串");
    let total = long.len();
    let head = format!(
        "<file name=\"big.log\">\nOnly the first 65536 of {total} bytes of this file are shown.\n{}\n</file>\n",
        "a".repeat(65_536)
    );
    assert_eq!(said, head);
    assert!(!said.contains("尾巴"), "截掉的不给");
}

#[test]
fn a_text_file_in_a_tool_result_goes_into_its_content() {
    let request = request(vec![
        Message::User {
            blocks: vec![text("读一下")],
        },
        Message::Assistant {
            blocks: vec![tool_call(
                "call_2_1",
                "read",
                r#"{"path":"notes.md"}"#,
                None,
            )],
        },
        Message::Tool {
            call_id: id("call_2_1"),
            error: false,
            blocks: vec![file(NOTES, "notes.md", "text/markdown")],
        },
    ]);
    let body = encoded(&request, Inputs::default(), &texts()).expect("blob 都在");
    assert_eq!(
        content(&body, 3),
        "<file name=\"notes.md\">\n# 待办\n- 写测试\n- 跑 CI\n</file>\n"
    );
}

#[test]
fn an_older_snapshot_writes_placeholders_for_text_files_too() {
    // 以前造的快照里没有那三句，`file-omitted` 也没有大小：照当时的样子写。
    let old = DriverTexts::new(DriverTextSources {
        image_omitted: include_str!("../../../resources/core/drivers/image-omitted.txt"),
        file_omitted: "A file was attached here ({name}, {media_type}), but this model cannot read it.\n",
        no_output: include_str!("../../../resources/core/drivers/no-output.txt"),
        tool_attachments: include_str!("../../../resources/core/drivers/tool-attachments.txt"),
        tool_attachments_only: include_str!(
            "../../../resources/core/drivers/tool-attachments-only.txt"
        ),
        text_file: None,
    })
    .expect("用得了");
    let body = encoded(&attached(), Inputs::default(), &old).expect("blob 都在");
    let said = content(&body, 1);
    let said = said.as_str().expect("一个字符串");
    assert!(
        said.contains(
            "A file was attached here (notes.md, text/markdown), but this model cannot read it.\n"
        ),
        "{said}"
    );
    assert!(!said.contains("<file"), "{said}");
}

#[test]
fn a_file_whose_blob_is_missing_is_reported() {
    let error = encode(
        &attached(),
        &call(Inputs::default(), None),
        &Compat::default(),
        &texts(),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(error, EncodeError::MissingBlob(ContentHash::of(NOTES)));
}
