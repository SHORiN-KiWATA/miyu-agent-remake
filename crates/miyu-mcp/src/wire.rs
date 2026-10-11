//! 一行一条的 JSON-RPC 2.0（`docs/designs/04-核心协议.md` 第三节同样的分帧，MCP 的标准输入输出也是这样）：读一行、认它是
//! 回应、请求还是通知。

use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

/// 一行最多多少字节：64 MiB。图片是 base64 写在一行里的，大一点的截图也装得下；再长的当这条连接坏了。
pub(crate) const LINE_LIMIT: usize = 64 * 1024 * 1024;

/// 读到的一行。
#[derive(Debug)]
pub(crate) enum Read {
    /// 一行，去掉了行尾。
    Line(Vec<u8>),
    /// 读到头了。
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

/// 服务发来的一条。
#[derive(Debug, PartialEq)]
pub(crate) enum Incoming {
    /// 对我们第 `id` 个请求的回应：成了的结果，或者错。
    Answer(u64, Result<Value, Value>),
    /// 服务反过来的请求：编号原样（回的时候照抄）、方法。
    Request(Value, String),
    /// 通知：方法、参数。
    Notice(String, Value),
    /// 认不出来的：说哪里不对。
    Bad(String),
}

/// 把一行认成 [`Incoming`]。
pub(crate) fn parse(line: &[u8]) -> Incoming {
    let Ok(Value::Object(mut object)) = serde_json::from_slice::<Value>(line) else {
        return Incoming::Bad("not a JSON object".to_string());
    };
    let id = object.remove("id");
    let method = object
        .remove("method")
        .and_then(|method| method.as_str().map(str::to_string));
    match (id, method) {
        (Some(id), Some(method)) => Incoming::Request(id, method),
        (None, Some(method)) => Incoming::Notice(
            method,
            object.remove("params").unwrap_or(Value::Object(Map::new())),
        ),
        (Some(id), None) => {
            let Some(id) = id.as_u64() else {
                return Incoming::Bad(format!("an answer to an id we never used: {id}"));
            };
            match (object.remove("result"), object.remove("error")) {
                (Some(result), None) => Incoming::Answer(id, Ok(result)),
                (None, Some(error)) => Incoming::Answer(id, Err(error)),
                _ => Incoming::Bad(format!("answer {id} has neither result nor error")),
            }
        }
        (None, None) => {
            Incoming::Bad("neither a request, an answer nor a notification".to_string())
        }
    }
}

/// 一个请求，带结尾的换行。
pub(crate) fn request(id: u64, method: &str, params: Value) -> Vec<u8> {
    line(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
}

/// 一个通知，带结尾的换行。
pub(crate) fn notice(method: &str, params: Value) -> Vec<u8> {
    line(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
}

/// 回服务反过来的请求：成了的结果，或者错。
pub(crate) fn answer(id: &Value, outcome: Result<Value, (i64, &str)>) -> Vec<u8> {
    match outcome {
        Ok(result) => line(&json!({"jsonrpc": "2.0", "id": id, "result": result})),
        Err((code, message)) => line(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": code, "message": message},
        })),
    }
}

/// 测试用的假服务反过来问客户端的请求：编号是字。
#[cfg(any(test, feature = "testkit"))]
pub(crate) fn request_raw(id: &str, method: &str) -> Vec<u8> {
    line(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": {}}))
}

/// 测试用的假服务回一个带附带的错。
#[cfg(any(test, feature = "testkit"))]
pub(crate) fn answer_with(id: &Value, code: i64, message: &str, data: Value) -> Vec<u8> {
    line(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message, "data": data},
    }))
}

/// 写成一行：JSON 里的换行都转义过，不会断开。
fn line(value: &Value) -> Vec<u8> {
    let mut bytes = value.to_string().into_bytes();
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
mod tests;
