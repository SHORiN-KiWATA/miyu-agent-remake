//! 几个测试共用的：一份请求字节、一个驱动、驱动的流的样本。假服务器在 `miyu_http::testkit`。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::path::PathBuf;

use miyu_drivers::openai_chat::Compat;
use miyu_drivers::{DriverTextSources, DriverTexts, OpenAiChat};

/// 一份请求字节：内容无所谓，查的是一字不差地发出去。里面的「你好」也拿来查日志里没有正文。
pub const BODY: &[u8] =
    r#"{"model":"deepseek-v4","messages":[{"role":"user","content":"你好"}],"stream":true}"#
        .as_bytes();

/// OpenAI 兼容的驱动，文字都是占位。
pub fn driver() -> OpenAiChat {
    let texts = DriverTexts::new(DriverTextSources {
        image_omitted: "image\n",
        file_omitted: "file {name} {media_type}\n",
        no_output: "nothing\n",
        tool_attachments: "attachments\n",
        tool_attachments_only: "only\n",
        text_file: None,
    })
    .expect("占位用得了");
    OpenAiChat::new(Compat::default(), texts)
}

/// 驱动的流的样本。
pub fn sample(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/openai-chat/streams")
        .join(format!("{name}.sse"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()))
}
