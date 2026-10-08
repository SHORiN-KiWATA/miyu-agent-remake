//! 小程序的协议（施工 R-5 上，`docs/blueprint/recall.md` 第四条第 4 款）：标准输入一行 `{"id":"…","text":"…"}`，标准输出
//! 回一行 `{"id":"…","vector":[…]}`；读不懂的、算不出的回 `{"id":"…","error":"…"}`（读不出 `id` 的写 `null`），接着读下一行。
//! 一次一条，读完一行、回完一行再读下一行。只有核心读它，不给模型看：原话是英文短句，不进登记簿。

use std::io::{self, BufRead, Write};

use serde::Serialize;
use serde_json::Value;

use crate::model::Embedder;

/// 一行回应。
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Reply {
    /// 算出来了：f32，JSON 的最短写法，读回 f32 一位不差。
    Vector { id: String, vector: Vec<f32> },
    /// 读不懂、算不出。
    Error { id: Option<String>, error: String },
}

/// 一行一行读 `input`，每行回一行到 `output`，`input` 读完了交回。
///
/// # Errors
///
/// 读不了 `input`、写不进 `output`（核心那一头关了）。
pub fn serve(
    embedder: &mut Embedder,
    input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    for line in input.split(b'\n') {
        let reply = answer(embedder, &line?);
        serde_json::to_writer(&mut output, &reply).map_err(io::Error::other)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

/// 回一行请求（行尾的 `\r` 是 JSON 的空白，照常读）。
fn answer(embedder: &mut Embedder, line: &[u8]) -> Reply {
    let error = |id: Option<&str>, error: String| Reply::Error {
        id: id.map(str::to_string),
        error,
    };
    let Ok(line) = std::str::from_utf8(line) else {
        return error(None, "bad request: not UTF-8".to_string());
    };
    let request: Value = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(why) => return error(None, format!("bad request: {why}")),
    };
    let Some(id) = request.get("id").and_then(Value::as_str) else {
        return error(None, "bad request: no id".to_string());
    };
    let Some(text) = request.get("text").and_then(Value::as_str) else {
        return error(Some(id), "bad request: no text".to_string());
    };
    match embedder.embed(text) {
        Ok(vector) => Reply::Vector {
            id: id.to_string(),
            vector,
        },
        Err(why) => error(Some(id), why.to_string()),
    }
}
