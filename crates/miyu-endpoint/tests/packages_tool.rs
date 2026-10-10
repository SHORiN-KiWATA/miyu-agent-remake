//! 她看软件包（施工 F-10 上，设计 `31-软件包.md` 第六节）：真核心的会话调 `packages`，经协议端点交给会话的端口看：列出、看一个、
//! 看一个包文件夹，和协议的 `package.list`、`package.info`、装之前看一眼是同一份，名字照英文；看文件夹什么都不装。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 一个界面包。
const PANE: &str = "[package]\nprotocol = [1, 1]\nversion = \"2\"\nname = { en = \"Pane\", zh = \"面板\" }\n\n[ui]\n";

/// 会话里每一条工具结果的字，照先后。
fn home_results(home: &Home, session: &str) -> Vec<String> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_session_looks_at_packages_through_the_core() {
    let home = Home::new();
    let folder = home.work.join("pane");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), PANE).expect("写得进");
    let script = Script::new([
        Play::calls(&[("packages", "{}")]),
        Play::calls(&[("packages", r#"{"package":"basesystem"}"#)]),
        Play::calls(&[("packages", &json!({"path": folder}).to_string())]),
        Play::Says("看完了。"),
    ]);
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
        .expect("合写法");
    let core = Arc::new(home.core_full(&script, tools, None, TOKEN));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "看看装了什么").await;
    home.until_turns(&session, 1).await;
    let results = home_results(&home, &session);
    assert_eq!(results.len(), 3, "{results:?}");
    assert!(
        results[0]
            .lines()
            .any(|line| line.starts_with("basesystem: Base system.")),
        "列出来一个一行，名字照英文：{}",
        results[0]
    );
    let info: serde_json::Value =
        serde_json::from_str(results[1].trim_end()).unwrap_or_else(|_| panic!("{}", results[1]));
    assert_eq!(
        (
            info["package"].clone(),
            info["name"].clone(),
            info["layer"].clone()
        ),
        (json!("basesystem"), json!("Base system"), json!("shipped")),
        "{info}"
    );
    let (lead, preview) = results[2].split_once('\n').unwrap_or_default();
    assert_eq!(
        lead,
        format!(
            "{} is a valid package. Installing it gives:",
            folder.display()
        )
    );
    let preview: serde_json::Value =
        serde_json::from_str(preview.trim_end()).unwrap_or_else(|_| panic!("{}", results[2]));
    assert_eq!(
        (
            preview["package"].clone(),
            preview["version"].clone(),
            preview["program"].clone()
        ),
        (json!("pane"), json!("2"), json!("ui"))
    );
    assert!(
        !home.root.path().join("home/alice/packages/pane").exists(),
        "看一眼不装"
    );
    drop(client);
    drop(core);

    // 核心重启过，载入的会话照样看得到（端口在载入时交进来）。
    let again = Script::new([
        Play::calls(&[("packages", r#"{"package":"net"}"#)]),
        Play::Says("嗯。"),
    ]);
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
        .expect("合写法");
    let mut client = Client::connect(Arc::new(home.core_full(&again, tools, None, TOKEN)));
    client.hello().await;
    client.say("s2", &session, "再看一个").await;
    home.until_turns(&session, 2).await;
    let last = home_results(&home, &session);
    assert!(
        last.last()
            .is_some_and(|text| text.contains(r#""package":"net""#)),
        "{last:?}"
    );
}
