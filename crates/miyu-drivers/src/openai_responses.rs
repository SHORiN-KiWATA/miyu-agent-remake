//! OpenAI 的 Responses 接口（`docs/blueprint/drivers/openai-responses.md`，施工 8-13）：OpenAI 官方、opencode Zen 上的 GPT
//! 说这一种。
//!
//! 不存对话：每次请求带全部历史、`"store":false`，不用 `previous_response_id`，对话的真相只在我们的日志里。统一的请求照那一页
//! 写成一个 JSON：顶层照 `model`、`instructions`、`input`、`tools`、`store`、`stream`、`max_output_tokens`、思考强度的先后，
//! `input` 里一项一项照 `openai_responses/input.rs` 写，同样的输入字节一定一样。缓存是自动的、按前缀，不打点。响应是 SSE
//! 流，[`Decoder`] 解成内核的四种增量。列模型和 openai-chat 一样（[`crate::openai_chat::parse_models`]）。

mod decode;
mod effort;
mod input;
mod usage;
mod wire;

pub use decode::Decoder;

use std::collections::BTreeSet;

use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Request;
use serde::Serialize;

pub use crate::{EncodeError, Encoded};

use crate::{BlobBytes, Call, DriverTexts, media};

/// 驱动家族：私有数据里写的是它的，才是这个驱动的（`03-事件模型.md` 第九节）。
pub const FAMILY: &str = "openai-responses";

/// 请求发到供应商地址后面的这一截。
pub const PATH: &str = "/responses";

/// 编码：顶层照 `model`、`instructions`、`input`、`tools`、`"store":false`、`"stream":true`、`max_output_tokens`、思考强度的
/// 先后写，别的字段一概不发。带着接着写记号的照原样发（这一家没有前缀续写），发到 [`PATH`]。
///
/// # Errors
///
/// 要用的 blob 执行器没交进来：先照 [`blobs_needed`] 取。
///
/// # Panics
///
/// 实际不会 panic：线上的消息里没有写不成 JSON 的东西。
pub fn encode(
    request: &Request,
    call: &Call,
    texts: &DriverTexts,
    blobs: &dyn BlobBytes,
) -> Result<Encoded, EncodeError> {
    let items = input::write(request, call, texts, blobs)?;
    let mut body = Vec::new();
    body.extend_from_slice(b"{\"model\":");
    json(&mut body, call.model.as_str());
    if !request.system.is_empty() {
        body.extend_from_slice(b",\"instructions\":");
        json(&mut body, &request.system);
    }
    body.extend_from_slice(b",\"input\":[");
    let mut ranges = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            body.push(b',');
        }
        let start = body.len();
        json(&mut body, item);
        ranges.push(start..body.len());
    }
    body.push(b']');
    if let Some(tools) = wire::tools(request) {
        body.extend_from_slice(b",\"tools\":");
        json(&mut body, &tools);
    }
    body.extend_from_slice(b",\"store\":false,\"stream\":true");
    if let Some(limit) = call.max_output {
        body.extend_from_slice(format!(",\"max_output_tokens\":{limit}").as_bytes());
    }
    effort::write(&mut body, call.effort.as_deref());
    body.push(b'}');
    Ok(Encoded {
        body,
        messages: ranges,
        path: PATH.to_string(),
    })
}

/// 这份请求编码时要用哪些 blob：同 openai-chat（`media.rs`）。
pub fn blobs_needed(request: &Request, call: &Call) -> BTreeSet<ContentHash> {
    media::blobs_needed(request, call)
}

/// 写成紧凑的 JSON，接在后面。
fn json(body: &mut Vec<u8>, value: &(impl Serialize + ?Sized)) {
    serde_json::to_writer(body, value).expect("线上的消息里没有写不成 JSON 的东西");
}
