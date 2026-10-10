//! 扩展登记的工具要归功能（施工 T-2，设计 `30-插件框架.md` 第十二节，`docs/blueprint/providers.md`「怎么走」第 3 条）：清单写了
//! 几个功能的，登记了哪个功能都没列的工具，整个不收，`bad_tool`（`problem` 是 `feature`）；只登记列了的照收。写了空的
//! `[features]` 的一件都不收。只写了一个功能的（接入QQ 那样不列工具的）都归它；没写 `[features]` 的整个包算一个功能，照旧
//! （`provide.rs`）。归法同预设认工具归哪个功能（`Features::of_tool`）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

fn spec(name: &str) -> Value {
    json!({"name": name, "description": format!("The {name} tool."),
           "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]})
}

/// 包 `id` 的清单：拉起照 `steps`，功能表照 `features`（原样接在后面）。
fn install_featured(home: &Home, id: &str, program: &str, steps: &[String], features: &str) {
    let args: Vec<String> = steps.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        &format!("home/alice/packages/{id}.toml"),
        &format!(
            "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = {{ en = \"Echo\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"E\" }}\n\n[process]\nargs = [{}]\nstart = \"always\"\n\n{features}",
            args.join(", ")
        ),
    );
}

/// 等到 `path` 里记下的有 `n` 行，交回它们。
async fn lines(path: &std::path::Path, n: usize) -> Vec<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<String> = read(path).lines().map(str::to_string).collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 照清单 `features` 拉起一个先登记 `first`、再登记 `second` 的扩展，交回两次的回应。
async fn provided(features: &str, first: Value, second: Value) -> (Value, Value) {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "echo");
    install_featured(
        &home,
        "echo",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{first}"),
            &format!("ask:provide:{second}"),
            "serve",
        ]),
        features,
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let got = lines(&path, 4).await;
    core.stop_extensions().await;
    let read = |line: &str| -> Value { serde_json::from_str(line).expect("回应是 JSON") };
    (read(&got[2]), read(&got[3]))
}

#[tokio::test]
async fn a_tool_outside_every_feature_is_refused() {
    let features = "[features.echo]\nname = { en = \"Echo\" }\ntools = [\"echo_back\"]\n\n[features.more]\nname = { en = \"More\" }\ntools = [\"echo_more\"]\n";
    let (stray, listed) = provided(
        features,
        json!({"tools": [spec("echo_back"), spec("stray")]}),
        json!({"tools": [spec("echo_back")]}),
    )
    .await;
    assert_eq!(stray["error"]["data"]["reason"], "bad_tool", "{stray}");
    assert_eq!(stray["error"]["data"]["tool"], "stray", "{stray}");
    assert_eq!(stray["error"]["data"]["problem"], "feature", "{stray}");
    assert_eq!(listed["result"], json!({"tools": 1}), "{listed}");
}

#[tokio::test]
async fn an_empty_feature_table_takes_no_tools() {
    let (one, none) = provided(
        "[features]\n",
        json!({"tools": [spec("echo_back")]}),
        json!({"tools": []}),
    )
    .await;
    assert_eq!(one["error"]["data"]["problem"], "feature", "{one}");
    assert_eq!(none["result"], json!({"tools": 0}), "{none}");
}

#[tokio::test]
async fn a_single_feature_takes_every_tool() {
    let (both, one) = provided(
        "[features.echo]\nname = { en = \"Echo\" }\n",
        json!({"tools": [spec("echo_back"), spec("stray")]}),
        json!({"tools": [spec("echo_back")]}),
    )
    .await;
    assert_eq!(both["result"], json!({"tools": 2}), "{both}");
    assert_eq!(one["result"], json!({"tools": 1}), "{one}");
}
