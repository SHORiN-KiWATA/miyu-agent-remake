//! 还没开会话时换工作区（蓝图 `tui.md`「新会话：人格、工作区」第 4 条，核心 9-7 下）：先 `fs.realpath` 换成真实的位置，
//! 再 `fs.list` 验它是个能用的目录；成了的造会话时放进 `session.create` 的 `cwd`。开着的会话走 `command.run`，不经这里。

use serde_json::{Value, json};

use super::Refusal;
use super::rpc::{Failure, Rpc};

/// 验 `path`（相对的照终端所在的目录 `cwd` 接）：交回真实的位置，或者核心拒绝的原因。连接断了是外面那层 `Err`。
pub(super) async fn check(
    rpc: &mut Rpc,
    path: &str,
    cwd: &str,
) -> Result<Result<String, Refusal>, Failure> {
    let real = rpc
        .reply("fs.realpath", json!({"path": path, "cwd": cwd}))
        .await?;
    if let Some(refusal) = refused(&real) {
        return Ok(Err(refusal));
    }
    let Some(dir) = real["result"]["path"].as_str().map(str::to_string) else {
        return Ok(Err(Refusal {
            reason: None,
            message: String::new(),
            data: Value::Null,
        }));
    };
    let listed = rpc.reply("fs.list", json!({"cwd": dir})).await?;
    Ok(refused(&listed).map_or(Ok(dir), Err))
}

/// 回应是拒绝的：原因码、原话。
fn refused(message: &Value) -> Option<Refusal> {
    let error = message.get("error")?;
    Some(Refusal {
        reason: error["data"]["reason"].as_str().map(str::to_string),
        message: error["message"].as_str().unwrap_or_default().to_string(),
        data: error["data"].clone(),
    })
}
