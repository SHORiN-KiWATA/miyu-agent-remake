//! 附件（蓝图 `tui.md`「输入框」第 12 条，`protocol.md` 的 `blob.put`）：发话以前把每个文件交给核心，交回的回应放进
//! `session.send` 的 `attachments`，从本机文件来的另带原来的路径 `path`（核心 3-9 五补：模型看不了时占位那一句带上它；剪贴板贴的
//! 图存在临时目录里，不带）。传不上的（太大、读不了、在数据根里）整句不发，交回核心的原话。

use std::path::{Path, PathBuf};

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
        out.push(with_path(got, file));
    }
    Ok(out)
}

/// `blob.put` 的回应接上原来的路径（核心 3-9 五补）。
/// 剪贴板贴的图（暂存在临时目录）、写法核心不收的（不是绝对路径、有控制字符、超过 4096 字节）不带。
fn with_path(mut got: Value, file: &Path) -> Value {
    let text = file.display().to_string();
    let fits = file.is_absolute() && text.len() <= 4096 && !text.chars().any(char::is_control);
    if fits && !crate::clipboard::is_staged(file) {
        got["path"] = json!(text);
    }
    got
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use super::with_path;

    #[test]
    fn a_file_from_disk_carries_its_path_and_a_pasted_one_does_not() {
        let got = json!({"blob": "sha256:aa", "name": "晚霞.png"});
        let from_disk = with_path(got.clone(), Path::new("/home/me/图/晚霞.png"));
        assert_eq!(from_disk["path"], "/home/me/图/晚霞.png");
        assert_eq!(from_disk["blob"], "sha256:aa", "回应原样留着");
        let pasted = Path::new("/home/me/.cache/miyu/tui/pasted/4242/0a1b.png");
        assert!(
            with_path(got.clone(), pasted).get("path").is_none(),
            "剪贴板贴的不带"
        );
        let odd = Path::new("/tmp/a\nb.png");
        assert!(
            with_path(got, odd).get("path").is_none(),
            "有控制字符的不带，核心会拒"
        );
    }
}
