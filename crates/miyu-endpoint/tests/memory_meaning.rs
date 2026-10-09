//! 人经协议照意思找记忆（施工 R-5 下，`docs/blueprint/recall.md` 第三条）：真核心接上本机 embedding（手造的小模型、真的
//! `miyu-embed`，文件事先放在缓存目录里），`memory.search` 第一次只走关键词，搜的时候起的后台补齐向量以后照意思找得到。
//! `off` 的那一路和她的工具共用一个判法（`miyu_session::by_meaning`），在会话的测试里。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_session::EmbedSetup;
use miyu_session::testkit::Script;

use crate::support::memories::texts;
use crate::support::*;

fn tiny() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../miyu-embed/tests/fixtures/tiny")
}

/// cargo 编出来的 `miyu-embed`：和测试程序所在的 `deps/` 同一层。
fn program() -> PathBuf {
    let exe = std::env::current_exe().expect("知道测试程序在哪");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("在 target/<profile>/deps/ 里");
    let program = dir.join(format!("miyu-embed{}", std::env::consts::EXE_SUFFIX));
    assert!(
        program.is_file(),
        "{} 不在：先 cargo build -p miyu-embed（cargo test --workspace 会编它）",
        program.display()
    );
    program
}

/// 接上小模型的核心：系统配置里默认人格是软件工程师。
fn core(home: &Home) -> Arc<miyu_endpoint::Core> {
    home.write("system/config.toml", "[persona]\ndefault = \"engineer\"\n");
    let cache = home.work.join("cache").join("embed");
    std::fs::create_dir_all(cache.join("tiny")).expect("建得了");
    for name in ["model.onnx", "vocab.txt"] {
        std::fs::copy(tiny().join(name), cache.join("tiny").join(name)).expect("放得下");
    }
    let setup = EmbedSetup {
        program: Some(program()),
        manifest: tiny().join("manifest.toml"),
        cache: Some(cache),
        client: miyu_http::fetcher(miyu_http::Proxy::Off).expect("造得出"),
        idle: Duration::from_secs(600),
    };
    let configured = venues::configured_core(home, &Script::new([]), miyu_tool::Catalog::default());
    let core = Arc::try_unwrap(configured).unwrap_or_else(|_| panic!("刚造的只有一份"));
    Arc::new(core.with_embedder(setup))
}

#[tokio::test]
async fn search_finds_by_meaning_once_the_vectors_are_filled() {
    let home = Home::new();
    let mut client = Client::connect(core(&home));
    client.hello().await;
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "我的猫"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    let first = client
        .call("c2", "memory.search", json!({"query": "喝茶"}))
        .await;
    assert!(texts(&first).is_empty(), "第一次只走关键词：{first}");
    let mut found = Vec::new();
    for n in 0..600 {
        let reply = client
            .call(&format!("s{n}"), "memory.search", json!({"query": "喝茶"}))
            .await;
        found = texts(&reply);
        if !found.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        found,
        [("m1".to_string(), "我的猫".to_string())],
        "照意思找得到"
    );
}
