//! `human.get`（施工 W-1，`docs/blueprint/web-module.md`「二、给人看的字」、`protocol.md`「`human.get`」）：工具的
//! 样子、说法的模板原文，和 `Human::load` 读到的一样；软件包盖掉内核的同名工具；没有这种语言照英文；不写 `language`
//! 照握手的语言；`language` 不合写法是参数不对；回应里没有 `config` 那一格；读不懂是内部出错；改了资源，下一次调
//! 就是新的；真核心照源码树的资源，三种语言都交得出来。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

use crate::support::*;

/// 一个用完就删的临时资源目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(
            std::env::temp_dir().join(format!("miyu-endpoint-human-{}-{n}", std::process::id())),
        )
    }

    fn file(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, text).expect("写得进");
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 内核一份、软件包一份，内核英文另有一份不同的，软件包只有中文：覆盖、前缀、英文退回都用得上。
fn fixture() -> Scratch {
    let scratch = Scratch::new();
    scratch.file(
        "core/human/zh.json",
        r#"{"tools":{"t":{"name":"核心的","icon":"★"}},"said":{"a":"core {x}"}}"#,
    );
    scratch.file(
        "core/human/en.json",
        r#"{"tools":{"t":{"name":"Core"}},"said":{"a":"core en {x}"}}"#,
    );
    scratch.file(
        "software/pkg/human/zh.json",
        r#"{"tools":{"t":{"name":"包的"}},"said":{"b":"pkg {y}"}}"#,
    );
    scratch
}

/// 握手，交回客户端。
async fn connected(core: std::sync::Arc<miyu_endpoint::Core>) -> Client {
    let mut client = Client::connect(core);
    client.hello().await;
    client
}

#[tokio::test]
async fn matches_human_load_and_package_overrides_core() {
    let fixture = fixture();
    let home = Home::new();
    let mut client = connected(home.core_with_resources(&Script::new([]), fixture.0.clone())).await;

    let reply = client
        .call("h1", "human.get", json!({"language": "zh"}))
        .await;
    let result = &reply["result"];
    assert_eq!(result["language"], "zh");

    // 和 `Human::load` 直接读到的一字不差。
    let loaded = Human::load(&ResourceRoot::at(&fixture.0), "zh").expect("读得出来");
    let expected_tools = serde_json::to_value(loaded.tools()).expect("序列化得了");
    let expected_said: std::collections::BTreeMap<&str, &str> = loaded.said_entries().collect();
    assert_eq!(result["tools"], expected_tools);
    assert_eq!(
        result["said"],
        serde_json::to_value(&expected_said).unwrap()
    );

    // 软件包盖掉内核的同名工具。
    assert_eq!(result["tools"]["t"]["name"], "包的");
    assert_eq!(result["tools"]["t"]["icon"], Value::Null, "软件包没写符号");

    // 说法的编号带着它在资源目录里的位置，模板是原文、一个字不换（没有填字段）。
    assert_eq!(result["said"]["core/a"], "core {x}");
    assert_eq!(result["said"]["software/pkg/b"], "pkg {y}");

    // 回应里没有 `config` 那一格。
    assert!(result.get("config").is_none());
}

#[tokio::test]
async fn falls_back_to_english_when_the_language_is_missing() {
    let fixture = fixture();
    let home = Home::new();
    let mut client = connected(home.core_with_resources(&Script::new([]), fixture.0.clone())).await;

    let en = client
        .call("h1", "human.get", json!({"language": "en"}))
        .await;
    // `fr` 合写法，但哪一份都没有这种语言：照英文退，内容和 `en` 一样；软件包（只有中文）两边都没有。
    let fr = client
        .call("h2", "human.get", json!({"language": "fr"}))
        .await;
    assert_eq!(fr["result"]["tools"], en["result"]["tools"]);
    assert_eq!(fr["result"]["said"], en["result"]["said"]);
    // 回应的 `language` 是要的那一种，不说哪一份退回了英文。
    assert_eq!(fr["result"]["language"], "fr");
    assert_eq!(en["result"]["tools"]["t"]["name"], "Core");
    assert!(
        en["result"]["said"].get("software/pkg/b").is_none(),
        "软件包只有中文，英文那边没有这一句"
    );
}

#[tokio::test]
async fn defaults_to_the_connection_language_when_language_is_unwritten() {
    let fixture = fixture();
    let home = Home::new();

    // 握手报中文：不写 `language` 照它。
    let mut zh_client =
        Client::connect(home.core_with_resources(&Script::new([]), fixture.0.clone()));
    zh_client.hello().await; // locale 是 zh-CN
    let zh = zh_client.call("h1", "human.get", json!({})).await;
    assert_eq!(zh["result"]["language"], "zh");
    assert_eq!(zh["result"]["tools"]["t"]["name"], "包的");

    // 不能让人输入的头没报语言：照英文。
    let mut en_client =
        Client::connect(home.core_with_resources(&Script::new([]), fixture.0.clone()));
    en_client.hello_without_input().await;
    let en = en_client.call("h1", "human.get", json!({})).await;
    assert_eq!(en["result"]["language"], "en");
}

#[tokio::test]
async fn a_badly_formed_language_is_bad_params() {
    let home = Home::new();
    let mut client = connected(home.core(&Script::new([]))).await;
    for bad in [
        json!(""),
        json!("a"),
        json!("abcdefghi"),
        json!("ZH"),
        json!("z1"),
        json!(5),
        json!([]),
    ] {
        let reply = client
            .call("h1", "human.get", json!({"language": bad}))
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{bad:?}");
    }
    // 格式对、没有这种语言的不算参数不对：照英文退。2 个字、8 个字（上下边界）都收。
    for (i, good) in ["xx", "abcdefgh"].into_iter().enumerate() {
        let ok = client
            .call(&format!("h2-{i}"), "human.get", json!({"language": good}))
            .await;
        assert!(ok.get("error").is_none(), "{good}: {ok:?}");
    }
}

#[tokio::test]
async fn unreadable_resources_are_an_internal_error() {
    let scratch = Scratch::new();
    scratch.file("core/human/zh.json", "{");
    let home = Home::new();
    let mut client = connected(home.core_with_resources(&Script::new([]), scratch.0.clone())).await;
    let reply = client
        .call("h1", "human.get", json!({"language": "zh"}))
        .await;
    assert_eq!(reason(&reply), Some("internal_error"));
}

#[tokio::test]
async fn resources_are_reread_on_every_call_without_restarting_the_core() {
    let scratch = Scratch::new();
    scratch.file("core/human/zh.json", r#"{"tools":{"t":{"name":"A"}}}"#);
    let home = Home::new();
    let mut client = connected(home.core_with_resources(&Script::new([]), scratch.0.clone())).await;

    let first = client
        .call("h1", "human.get", json!({"language": "zh"}))
        .await;
    assert_eq!(first["result"]["tools"]["t"]["name"], "A");

    scratch.file("core/human/zh.json", r#"{"tools":{"t":{"name":"B"}}}"#);
    let second = client
        .call("h2", "human.get", json!({"language": "zh"}))
        .await;
    assert_eq!(
        second["result"]["tools"]["t"]["name"], "B",
        "核心没重启也读到新的"
    );
}

#[tokio::test]
async fn the_real_core_gives_tools_and_said_in_all_three_languages() {
    let home = Home::new();
    let mut client = connected(home.core(&Script::new([]))).await;
    for language in ["zh", "en", "ja"] {
        let reply = client
            .call("h1", "human.get", json!({"language": language}))
            .await;
        let result = &reply["result"];
        assert_eq!(result["language"], language);
        let tools = result["tools"].as_object().expect("tools 是对象");
        let said = result["said"].as_object().expect("said 是对象");
        assert!(!tools.is_empty(), "{language} 的 tools 是空的");
        assert!(!said.is_empty(), "{language} 的 said 是空的");
        assert!(result.get("config").is_none());
    }
}
