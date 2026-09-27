//! 一行一条的 JSON-RPC 2.0（`docs/designs/04-核心协议.md` 第二节）：读一行，认成请求；回应写成一行。
//!
//! 请求的 `id` 一律是字符串，照命令编号的写法（第九节「先做的几样怎么写」）：命令的 `id` 就是命令编号。
//! 没有 `id` 的是通知，不回应。

use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

use miyu_kernel::id::CommandId;

use crate::refusal::{Locale, Refusal};

/// 一行最长多少字节：附件走 `blob.put`，一条消息用不着更长。超长的当读不懂，不在内存里攒。
pub(crate) const LINE_LIMIT: usize = 1024 * 1024;

/// 收到的一条请求。
#[derive(Debug)]
pub(crate) struct Request {
    /// 请求的 `id`，就是命令编号。
    pub(crate) id: CommandId,
    /// 方法。
    pub(crate) method: String,
    /// 参数；没写的是空对象。
    pub(crate) params: Value,
}

/// 读到的一行是什么。
#[derive(Debug)]
pub(crate) enum Incoming {
    /// 一条请求。
    Request(Request),
    /// 一条通知：头发来的通知现在一种都不认，不回应。
    Notification,
    /// 读不懂，或者不是请求：回这个拒绝，`id` 能认出来的照原样带回去。
    Bad(Value, Refusal),
}

/// 读一行读到了什么。
#[derive(Debug)]
pub(crate) enum Read {
    /// 一行，去掉了行尾。
    Line(Vec<u8>),
    /// 对方关了。
    Closed,
    /// 太长。
    TooLong,
}

/// 读一行，最多 [`LINE_LIMIT`] 字节。行尾的 `\n`（和它前面的 `\r`）去掉。
pub(crate) async fn read_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> std::io::Result<Read> {
    let mut line = Vec::new();
    let limit = u64::try_from(LINE_LIMIT)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let read = (&mut *reader)
        .take(limit)
        .read_until(b'\n', &mut line)
        .await?;
    if read == 0 {
        return Ok(Read::Closed);
    }
    if line.last() == Some(&b'\n') {
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
    } else if line.len() > LINE_LIMIT {
        return Ok(Read::TooLong);
    }
    Ok(Read::Line(line))
}

/// 把一行认成请求。
pub(crate) fn parse(line: &[u8]) -> Incoming {
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return Incoming::Bad(Value::Null, Refusal::PARSE);
    };
    let Value::Object(mut object) = value else {
        return Incoming::Bad(Value::Null, Refusal::INVALID);
    };
    let id = object.remove("id");
    let echo = match &id {
        Some(id @ (Value::String(_) | Value::Number(_))) => id.clone(),
        _ => Value::Null,
    };
    let method = match object.remove("method") {
        Some(Value::String(method)) => method,
        _ => return Incoming::Bad(echo, Refusal::INVALID),
    };
    if object.get("jsonrpc") != Some(&Value::String("2.0".to_string())) {
        return Incoming::Bad(echo, Refusal::INVALID);
    }
    let params = match object.remove("params") {
        None => Value::Object(serde_json::Map::new()),
        Some(params @ (Value::Object(_) | Value::Array(_))) => params,
        Some(_) => return Incoming::Bad(echo, Refusal::INVALID),
    };
    let id = match id {
        None => return Incoming::Notification,
        Some(Value::String(id)) => match CommandId::parse(&id) {
            Ok(id) => id,
            Err(_) => return Incoming::Bad(echo, Refusal::INVALID),
        },
        Some(_) => return Incoming::Bad(echo, Refusal::INVALID),
    };
    Incoming::Request(Request { id, method, params })
}

/// 接受的回应，写成一行（不带换行）。
pub(crate) fn result(id: &CommandId, result: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id.as_str(), "result": result}).to_string()
}

/// 拒绝的回应，写成一行（不带换行）：`code`、照头的语言写的话、`data.reason`。
pub(crate) fn error(id: Value, refusal: Refusal, locale: Locale) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": refusal.code,
            "message": refusal.message(locale),
            "data": {"reason": refusal.reason},
        },
    })
    .to_string()
}

#[cfg(test)]
mod tests;
