//! 把取来的字节存成核心的 blob（施工 O-33，`onebot.md` 第一条「平台工具（二）」第 3、4 条；`protocol.md` 的 `blob.open`、
//! `blob.write`、`blob.close`）：分块传，一块 [`CHUNK`] 字节（`blob.write` 一块最多 512 KiB），图大的也传得上去（`blob.put` 的
//! `data` 放在一行里，最多七百多 KiB）。回应同 `blob.put`：`blob`、`name`、`media_type`、`kind`，图另有 `width`、`height`。
//! 存在管理员名下；带进会话、工具结果里的，核心照会话的属主拷（`venues.md`「`venue.session`」第 3 条）。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use super::{Caller, Gone, reason};

/// 一块传多少字节：`blob.write` 的上限。
const CHUNK: usize = 512 * 1024;

/// 没存成。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Unstored {
    /// 核心拒了：原因码（`attachment_too_big` 这类）。
    Refused(String),
    /// 核心断开了。
    Gone,
}

/// 把 `bytes` 存成名字是 `name`（只是名字，合核心的文件名写法）的 blob，交回 `blob.close` 的 `result`。
///
/// # Errors
///
/// 核心拒了哪一步（交回原因码）、核心断开。
pub(crate) async fn put(caller: &Caller, name: &str, bytes: &[u8]) -> Result<Value, Unstored> {
    let opened = call(
        caller,
        "blob.open",
        json!({"name": name, "size": bytes.len()}),
    )
    .await?;
    let Some(upload) = opened["upload"].as_str() else {
        return Err(Unstored::Refused("no_upload".to_string()));
    };
    let mut offset = 0;
    for chunk in bytes.chunks(CHUNK) {
        let params = json!({"upload": upload, "offset": offset, "data": STANDARD.encode(chunk)});
        call(caller, "blob.write", params).await?;
        offset += chunk.len();
    }
    call(caller, "blob.close", json!({"upload": upload})).await
}

/// 调一步，交回 `result`；拒了的交回原因码（没有原因码的写 `error`）。
async fn call(caller: &Caller, method: &str, params: Value) -> Result<Value, Unstored> {
    let reply = caller
        .call(method, params)
        .await
        .map_err(|Gone| Unstored::Gone)?;
    match (reply.get("error"), reason(&reply)) {
        (None, _) => Ok(reply["result"].clone()),
        (Some(_), reason) => Err(Unstored::Refused(reason.unwrap_or("error").to_string())),
    }
}
