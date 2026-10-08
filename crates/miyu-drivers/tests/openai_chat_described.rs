//! 替它看的图怎么进请求（施工 8-17，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：不能看图、请求的 `described` 里有这张图
//! 的转述的，换成带标签的转述，照文字拼（带名字的标签写上名字）；没有转述的照旧是占位；能看图的照旧发原图，`described`
//! 一个字节都不影响；工具结果里的图一样；以前造的快照里没有标签的，照旧写占位。

use std::collections::BTreeMap;

use crate::support::{
    call, id, image, named_image, read_tool, sample, sees_all, text, texts, tool_call,
};
use miyu_drivers::openai_chat::{Compat, encode};
use miyu_drivers::{DriverTextSources, DriverTexts, Inputs, TextFileSources};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};
use serde_json::Value;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const SHOT: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rscreenshot";

/// 晚霞那张的转述：多行，带尖括号和引号，原样进请求。
const SUNSET: &str = "An orange sunset over the sea.\nA sign reads \"<CLOSED>\".";

fn request(messages: Vec<Message>, described: &[(&[u8], &str)]) -> Request {
    Request {
        tools: vec![read_tool()],
        system: "You are a helpful assistant.".to_string(),
        messages,
        stable: 0,
        continuation: false,
        described: described
            .iter()
            .map(|(content, text)| (ContentHash::of(content), (*text).to_string()))
            .collect(),
    }
}

fn body(request: &Request, inputs: Inputs, texts: &DriverTexts) -> Vec<u8> {
    let blobs: BTreeMap<ContentHash, Vec<u8>> = [PNG, SHOT]
        .into_iter()
        .map(|content| (ContentHash::of(content), content.to_vec()))
        .collect();
    encode(
        request,
        &call(inputs, None),
        &Compat::default(),
        texts,
        &blobs,
    )
    .expect("要用的 blob 都在")
    .body
}

/// 线上第 `n` 条消息的 `content`。
fn content(body: &[u8], n: usize) -> Value {
    let body: Value = serde_json::from_slice(body).expect("是 JSON");
    body["messages"][n]["content"].clone()
}

/// 一条 user：一句话、人附的晚霞（转述过）、截图（没转述过）、又一句话。
fn attached(described: &[(&[u8], &str)]) -> Request {
    request(
        vec![Message::User {
            blocks: vec![
                text("这两张哪张是晚霞？"),
                named_image(PNG, "晚霞.png", "image/png"),
                named_image(SHOT, "截图.png", "image/png"),
                text("只说文件名。"),
            ],
        }],
        described,
    )
}

#[test]
fn a_blind_model_reads_the_description_in_the_picture_s_place() {
    let body = body(&attached(&[(PNG, SUNSET)]), Inputs::default(), &texts());
    sample("image-descriptions", &body);
    assert_eq!(
        content(&body, 1),
        "这两张哪张是晚霞？\n\
         <image-description name=\"晚霞.png\">\n\
         An orange sunset over the sea.\nA sign reads \"<CLOSED>\".\n\
         </image-description>\n\
         An image was attached here (截图.png), but this model cannot view images.\n\
         只说文件名。"
    );
}

#[test]
fn a_model_that_sees_gets_the_pictures_whatever_is_described() {
    let plain = body(&attached(&[]), sees_all(), &texts());
    let described = body(&attached(&[(PNG, SUNSET)]), sees_all(), &texts());
    assert_eq!(described, plain, "能看图的不看转述，一个字节都不变");
}

/// 工具结果里的图（`read` 读出来的，不带名字）：不能看图的就地换成不带名字的标签和转述。
#[test]
fn a_described_picture_in_a_tool_result_stays_in_place() {
    let request = request(
        vec![
            Message::User {
                blocks: vec![text("截一张图")],
            },
            Message::Assistant {
                blocks: vec![tool_call("call_2_1", "read", r#"{"path":"a.png"}"#, None)],
            },
            Message::Tool {
                call_id: id("call_2_1"),
                error: false,
                blocks: vec![image(SHOT, "image/png")],
            },
        ],
        &[(SHOT, "A terminal.\n")],
    );
    let blind = body(&request, Inputs::default(), &texts());
    assert_eq!(
        content(&blind, 3),
        "<image-description>\nA terminal.\n</image-description>\n",
        "末尾有换行的不再补"
    );
}

#[test]
fn an_older_snapshot_writes_the_placeholder() {
    let old = DriverTexts::new(DriverTextSources {
        image_omitted: include_str!("../../../resources/core/drivers/image-omitted.txt"),
        file_omitted: include_str!("../../../resources/core/drivers/file-omitted.txt"),
        no_output: include_str!("../../../resources/core/drivers/no-output.txt"),
        tool_attachments: include_str!("../../../resources/core/drivers/tool-attachments.txt"),
        tool_attachments_only: include_str!(
            "../../../resources/core/drivers/tool-attachments-only.txt"
        ),
        text_file: Some(TextFileSources {
            file_open: include_str!("../../../resources/core/drivers/file-open.txt"),
            file_cut: include_str!("../../../resources/core/drivers/file-cut.txt"),
            file_close: include_str!("../../../resources/core/drivers/file-close.txt"),
        }),
        image_name: None,
        image_description: None,
    })
    .expect("用得了");
    let described = body(&attached(&[(PNG, SUNSET)]), Inputs::default(), &old);
    let plain = body(&attached(&[]), Inputs::default(), &old);
    assert_eq!(described, plain, "以前的快照照旧写占位");
}
