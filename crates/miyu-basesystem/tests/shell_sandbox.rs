//! `shell` 带了沙盒的（施工 5-1）：经助手起，助手收到的是 `run --spec <规格> -- <shell> <参数…>`。假助手是
//! `/bin/echo`：它把收到的参数印出来，就是命令的输出。没写成文件再执行，碰不上「文件正忙」。不带沙盒的照旧，见
//! `shell.rs`；三个平台上包出来的参数见 `src/shell/program/tests.rs`。

#![cfg(unix)]

mod support;

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_sandbox::{Sandboxed, Spec};

use support::Site;

/// 交回的字。
fn text(done: &miyu_tool::Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

#[tokio::test]
async fn a_sandboxed_call_runs_through_the_helper() {
    let site = Site::new();
    let spec = Spec::from_json(r#"{"write":["/work"]}"#).expect("读得懂");
    let json = spec.to_json().expect("写得成");
    let done = site
        .done_sandboxed(
            "shell",
            json!({ "command": "echo hi", "description": "Test" }),
            Sandboxed {
                helper: "/bin/echo".into(),
                spec,
                env: Vec::new(),
            },
        )
        .await;
    let printed = text(&done);
    assert!(!done.error, "{printed}");
    assert!(
        printed.starts_with(&format!("run --spec {json} -- /")),
        "助手、规格、分隔在前：{printed}"
    );
    assert!(
        printed.ends_with(" -c echo hi\n"),
        "shell 和它的参数在后：{printed}"
    );
}

/// 规格写不成 JSON（里面有不是 UTF-8 的路径）：说起不来，命令没跑，不会不经沙盒就跑。
#[tokio::test]
async fn a_spec_that_cannot_be_written_does_not_run_the_command() {
    use std::os::unix::ffi::OsStringExt;
    let site = Site::new();
    let mut spec = Spec::from_json("{}").expect("读得懂");
    spec.write
        .push(std::ffi::OsString::from_vec(vec![b'/', 0xff]).into());
    let done = site
        .done_sandboxed(
            "shell",
            json!({ "command": "touch ran", "description": "Test" }),
            Sandboxed {
                helper: "/bin/echo".into(),
                spec,
                env: Vec::new(),
            },
        )
        .await;
    assert!(done.error);
    let said = text(&done);
    assert!(said.starts_with("Could not run "), "{said}");
    assert!(!site.0.join("work/ran").exists(), "命令没跑");
}
