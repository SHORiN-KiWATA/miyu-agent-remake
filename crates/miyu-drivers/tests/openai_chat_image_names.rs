//! 带名字的图片怎么进请求（施工 3-9 四补，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：人附的图片带着文件名，能看图
//! 的前后各一段标签，不能看图的占位写上名字；不带名字的照旧（`media.json`、`tool-attachments.json` 这几份样本不变）；
//! 以前造的快照里没有那三句的，带名字的也照不带名字的写。

use std::collections::BTreeMap;

use crate::support::{
    call, id, image, named_image, read_tool, sample, sees_all, text, texts, tool_call,
};
use miyu_drivers::openai_chat::{Compat, encode};
use miyu_drivers::{DriverTextSources, DriverTexts, Inputs, TextFileSources};
use miyu_kernel::block::Block;
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const SHOT: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rscreenshot";

fn request(messages: Vec<Message>) -> Request {
    Request {
        tools: vec![read_tool()],
        system: "You are a helpful assistant.".to_string(),
        messages,
        stable: 0,
        continuation: false,
        described: Default::default(),
        output_cap: None,
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

/// 一段字的样子。
fn said(text: &str) -> Value {
    json!({"type": "text", "text": text})
}

/// 一条 user 消息：一句话、人附的两张图、又一句话。
fn attached() -> Request {
    request(vec![Message::User {
        blocks: vec![
            text("这两张哪张是晚霞？"),
            named_image(PNG, "晚霞.png", "image/png"),
            named_image(SHOT, "截图.png", "image/png"),
            text("只说文件名。"),
        ],
    }])
}

#[test]
fn named_images_are_wrapped_in_tags_with_their_names() {
    let body = body(&attached(), sees_all(), &texts());
    sample("image-names", &body);
    // 标签是文字，和挨着的字拼成一段。
    let parts = content(&body, 1);
    let parts = parts.as_array().expect("分成几段");
    assert_eq!(parts.len(), 5, "{parts:?}");
    assert_eq!(
        parts[0],
        said("这两张哪张是晚霞？\n<image name=\"晚霞.png\">\n")
    );
    assert_eq!(parts[1]["type"], "image_url");
    assert_eq!(parts[2], said("</image>\n<image name=\"截图.png\">\n"));
    assert_eq!(parts[3]["type"], "image_url");
    assert_eq!(parts[4], said("</image>\n只说文件名。"));
}

#[test]
fn a_model_that_cannot_see_gets_the_names_in_the_placeholders() {
    let body = body(&attached(), Inputs::default(), &texts());
    sample("image-names-omitted", &body);
    assert_eq!(
        content(&body, 1),
        "这两张哪张是晚霞？\n\
         An image was attached here (晚霞.png), but this model cannot view images.\n\
         An image was attached here (截图.png), but this model cannot view images.\n\
         只说文件名。"
    );
}

/// 工具结果里带名字的图（现在没有谁会造，写法一样）：能看图的连同标签一起挪到后面，不能看图的就地写带名字的占位。
#[test]
fn a_named_image_in_a_tool_result_keeps_its_name() {
    let request = request(vec![
        Message::User {
            blocks: vec![text("截一张图")],
        },
        Message::Assistant {
            blocks: vec![tool_call("call_2_1", "read", r#"{"path":"a.png"}"#, None)],
        },
        Message::Tool {
            call_id: id("call_2_1"),
            error: false,
            blocks: vec![
                named_image(PNG, "a.png", "image/png"),
                image(SHOT, "image/png"),
            ],
        },
    ]);
    let seen = body(&request, sees_all(), &texts());
    let moved = content(&seen, 4);
    let moved = moved.as_array().expect("分成几段");
    assert_eq!(moved.len(), 4, "{moved:?}");
    assert_eq!(
        moved[0],
        said(
            "These images and files were returned by the tool calls above.\n<image name=\"a.png\">\n"
        )
    );
    assert_eq!(moved[1]["type"], "image_url");
    assert_eq!(moved[2], said("</image>\n"));
    assert_eq!(moved[3]["type"], "image_url");
    let blind = body(&request, Inputs::default(), &texts());
    assert_eq!(
        content(&blind, 3),
        "An image was attached here (a.png), but this model cannot view images.\n\
         An image was attached here, but this model cannot view images.\n"
    );
}

/// 去掉名字的同一份请求。
fn without_names(request: &Request) -> Request {
    let mut request = request.clone();
    for message in &mut request.messages {
        let (Message::User { blocks }
        | Message::Assistant { blocks }
        | Message::Tool { blocks, .. }) = message;
        for block in blocks {
            if let Block::Image(image) = block {
                image.name = None;
            }
        }
    }
    request
}

#[test]
fn an_older_snapshot_writes_named_images_as_if_they_had_no_names() {
    // 以前造的快照里没有那三句：带名字的图和不带名字的一字不差。
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
        attached_path: None,
    })
    .expect("用得了");
    let plain = without_names(&attached());
    for inputs in [sees_all(), Inputs::default()] {
        assert_eq!(
            body(&attached(), inputs, &old),
            body(&plain, inputs, &old),
            "{inputs:?}"
        );
        // 出厂的这一份，不带名字的也照旧：前后什么都不加，占位不带名字。
        assert_eq!(body(&plain, inputs, &texts()), body(&plain, inputs, &old));
    }
}

/// 看不了的附件带上原来的路径（施工 3-9 五补）：从本机文件来的图、读不了的文件，占位写上路径；不带路径的照旧。
#[test]
fn a_model_that_cannot_see_gets_the_paths_in_the_placeholders() {
    let with_path = |block: Block, path: &str| -> Block {
        let path = Some(miyu_kernel::id::SourcePath::parse(path).expect("合写法"));
        match block {
            Block::Image(mut image) => {
                image.path = path;
                Block::Image(image)
            }
            Block::File(mut file) => {
                file.path = path;
                Block::File(file)
            }
            other => other,
        }
    };
    let request = request(vec![Message::User {
        blocks: vec![
            text("看看"),
            with_path(
                named_image(PNG, "晚霞.png", "image/png"),
                "/home/alice/晚霞.png",
            ),
            with_path(
                crate::support::file(SHOT, "报告.pdf", "application/pdf"),
                "~/报告.pdf",
            ),
        ],
    }]);
    let blind = body(&request, Inputs::default(), &texts());
    assert_eq!(
        content(&blind, 1),
        format!(
            "看看\n\
             An image was attached here (晚霞.png, from /home/alice/晚霞.png), but this model cannot view images.\n\
             A file was attached here (报告.pdf, application/pdf, {} bytes, from ~/报告.pdf), but this model cannot read it.\n",
            SHOT.len()
        )
    );
    let seen = body(&request, sees_all(), &texts());
    assert!(
        !String::from_utf8_lossy(&seen).contains("/home/alice"),
        "看得了的路径不进请求"
    );
}
