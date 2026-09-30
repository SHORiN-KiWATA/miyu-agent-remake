//! 附件（蓝图 `tui.md`「输入框」第 12 条，`protocol.md` 的 `blob.put`）：发话以前把每个文件交给核心，交回的回应原样放进
//! `session.send` 的 `attachments`。传不上的（太大、读不了、在数据根里）整句不发，交回核心的原话。

use std::path::PathBuf;

use serde_json::{Value, json};

use super::Update;
use super::rpc::{Failure, Rpc};

/// 照先后传每一个文件，交回它们的回应；有一个传不上就停，交回为什么。
pub(super) async fn attach(rpc: &mut Rpc, files: &[PathBuf]) -> Result<Vec<Value>, Update> {
    let mut out = Vec::with_capacity(files.len());
    for file in files {
        let params = json!({"path": file.display().to_string()});
        let got = rpc
            .call("blob.put", params)
            .await
            .map_err(|failure| match failure {
                Failure::Refused(message) => Update::Refused {
                    reason: None,
                    message,
                },
                Failure::Io(_) | Failure::Disconnected => Update::Disconnected,
            })?;
        out.push(got);
    }
    Ok(out)
}
