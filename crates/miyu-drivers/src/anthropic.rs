//! Anthropic 的消息接口（`docs/blueprint/drivers/anthropic.md`，施工 8-12）：Anthropic 官方、opencode Zen 上的 Claude 和走这种
//! 写法的兼容接口都说这一种。
//!
//! 统一的请求照那一页写成一个 JSON：顶层照 `model`、`max_tokens`、`system`、`tools`、`messages`、`stream`、思考强度的先后，
//! 相邻同角色的合成一条（`anthropic/messages.rs`），四处缓存打点照前缀契约放（`anthropic/marks.rs`），同样的输入字节一定
//! 一样。响应是 SSE 流，[`Decoder`] 解成内核的四种增量。列模型（[`parse_models`]）：`GET /models?limit=1000`。
//!
//! 这一家的写法只有一套，没有开关；思考的开关是接口自带的 `thinking`（`anthropic/effort.rs`）。

mod decode;
mod effort;
mod marks;
mod messages;
mod models;
mod usage;
mod wire;

pub use decode::Decoder;
pub use models::{MODELS_PATH, parse_models};

use std::collections::BTreeSet;

use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Request;
use serde::Serialize;

pub use crate::{EncodeError, Encoded};

use crate::{BlobBytes, Call, DriverTexts, media};

/// 驱动家族：私有数据里写的是它的，才是这个驱动的（`03-事件模型.md` 第九节）。
pub const FAMILY: &str = "anthropic";

/// 请求发到供应商地址后面的这一截。
pub const PATH: &str = "/messages";

/// 版本头 `anthropic-version` 的值。
pub const VERSION: &str = "2023-06-01";

/// `Call.max_output` 没有时写的输出上限：这一家一定要写，8192 是现役模型都收的保守值（图纸「起草时定的」第 9 条）。
pub const FALLBACK_MAX_TOKENS: u32 = 8192;

/// 打点的写法，一份请求里只有这一种（「缓存打点」第 3 条）：接在一块的最后一格后面。
const MARK: &[u8] = br#","cache_control":{"type":"ephemeral"}"#;

/// 编码：顶层照 `model`、`max_tokens`、`system`、`tools`、`messages`、`"stream":true`、思考强度的先后写，别的字段一概不发。
/// 带着接着写记号的照原样发（这一家不会接着写，图纸「起草时定的」第 16 条），发到 [`PATH`]。
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
    let written = messages::write(request, call, texts, blobs)?;
    let tools = wire::tools(request);
    let marks = marks::place(request, tools.as_deref(), &written);
    let mut body = Vec::new();
    body.extend_from_slice(b"{\"model\":");
    json(&mut body, call.model.as_str());
    let limit = call.max_output.unwrap_or(FALLBACK_MAX_TOKENS);
    body.extend_from_slice(format!(",\"max_tokens\":{limit}").as_bytes());
    if !request.system.is_empty() {
        body.extend_from_slice(b",\"system\":[");
        block(&mut body, &wire::Part::text(&request.system), marks.system);
        body.push(b']');
    }
    if let Some(tools) = &tools {
        body.extend_from_slice(b",\"tools\":[");
        for (index, tool) in tools.iter().enumerate() {
            if index > 0 {
                body.push(b',');
            }
            block(&mut body, tool, marks.tool == Some(index));
        }
        body.push(b']');
    }
    body.extend_from_slice(b",\"messages\":[");
    let mut ranges = Vec::with_capacity(written.messages.len());
    for (m, message) in written.messages.iter().enumerate() {
        if m > 0 {
            body.push(b',');
        }
        let start = body.len();
        body.extend_from_slice(b"{\"role\":");
        json(&mut body, message.role);
        body.extend_from_slice(b",\"content\":[");
        for (b, part) in message.content.iter().enumerate() {
            if b > 0 {
                body.push(b',');
            }
            block(&mut body, part, marks.messages.contains(&(m, b)));
        }
        body.extend_from_slice(b"]}");
        ranges.push(start..body.len());
    }
    body.extend_from_slice(b"],\"stream\":true");
    crate::write_temperature(&mut body, effort::temperature(call));
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

/// 去掉打点（「缓存打点」第 5 条）：查线上的前缀延伸时先去掉再比，打点每次往后挪，缓存的键不含它。去掉每一处
/// `,"cache_control":{"type":"ephemeral"}`、重算每条消息的位置。这一串字在 JSON 的字符串里出现不了：字符串里的引号都转义了。
#[cfg(any(test, feature = "testkit"))]
pub fn unmarked(encoded: &Encoded) -> Encoded {
    let mut body = Vec::with_capacity(encoded.body.len());
    // 原来的位置 → 去掉以后的位置：照去掉了几处往前挪。
    let mut cuts = Vec::new();
    let mut at = 0;
    while at < encoded.body.len() {
        if encoded.body[at..].starts_with(MARK) {
            cuts.push(at);
            at += MARK.len();
        } else {
            body.push(encoded.body[at]);
            at += 1;
        }
    }
    let moved = |position: usize| {
        position - cuts.iter().filter(|cut| **cut < position).count() * MARK.len()
    };
    Encoded {
        body,
        messages: encoded
            .messages
            .iter()
            .map(|range| moved(range.start)..moved(range.end))
            .collect(),
        path: encoded.path.clone(),
    }
}

/// 写一块：紧凑的 JSON；打了点的，在最后一格后面接上打点。每一块都是一个 JSON 对象，最后一个字节是 `}`。
fn block(body: &mut Vec<u8>, value: &impl Serialize, marked: bool) {
    json(body, value);
    if marked {
        body.pop();
        body.extend_from_slice(MARK);
        body.push(b'}');
    }
}

/// 写成紧凑的 JSON，接在后面。
fn json(body: &mut Vec<u8>, value: &(impl Serialize + ?Sized)) {
    serde_json::to_writer(body, value).expect("线上的消息里没有写不成 JSON 的东西");
}
