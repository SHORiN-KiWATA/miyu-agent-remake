//! 记忆照包启用（施工 R-10，`docs/blueprint/memory.md` 第十一条）：真核心经 `package.remove` 卸掉出厂的人格记忆，`memory.*`
//! 五个都回 `memory_not_installed`；`/remember` 回 `memory_unavailable`、为什么说没装，`command.catalog` 不列它（`/dream`
//! 也不列）；装回来以前记的都在。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::memories::{connected, pairs, texts};
use crate::support::*;

async fn remove(client: &mut Client) {
    let reply = client
        .call("r1", "package.remove", json!({"package": "memory"}))
        .await;
    assert_eq!(reply["result"]["removed"], true, "{reply}");
}

async fn restore(client: &mut Client) {
    let reply = client
        .call("i1", "package.install", json!({"package": "memory"}))
        .await;
    assert_eq!(reply["result"]["package"], "memory", "{reply}");
}

/// 命令表里列出来的名字。
fn names(reply: &Value) -> Vec<String> {
    reply["result"]["commands"]
        .as_array()
        .expect("有")
        .iter()
        .filter_map(|command| command["name"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn without_the_package_memory_methods_say_it_is_not_installed_and_nothing_is_lost() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "用户用 N 卡"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    remove(&mut client).await;
    for (id, method, params) in [
        ("a", "memory.list", json!({})),
        ("b", "memory.search", json!({"query": "N卡"})),
        (
            "c",
            "memory.remember",
            json!({"class": "user", "text": "又一条"}),
        ),
        ("d", "memory.update", json!({"id": "m1", "text": "改"})),
        ("e", "memory.forget", json!({"id": "m1", "why": "不对"})),
    ] {
        let reply = client.call(id, method, params).await;
        assert_eq!(
            reason(&reply),
            Some("memory_not_installed"),
            "{method}：{reply}"
        );
        assert_eq!(reply["error"]["message"], "没装人格记忆。", "{reply}");
    }
    restore(&mut client).await;
    let listed = client.call("l1", "memory.list", json!({})).await;
    assert_eq!(
        texts(&listed),
        pairs(&[("m1", "用户用 N 卡")]),
        "以前记的都在"
    );
}

#[tokio::test]
async fn without_the_package_remember_is_refused_and_not_listed() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let session = client.create("c1", "~").await;
    remove(&mut client).await;
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/remember 用户喜欢猫"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("memory_unavailable"), "{reply}");
    assert_eq!(reply["error"]["data"]["why"], "没装人格记忆。");
    let reply = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    assert_eq!(names(&reply), ["clear", "stop", "workspace"], "{reply}");
    restore(&mut client).await;
    let reply = client
        .call("c3", "command.catalog", json!({"session": session}))
        .await;
    assert_eq!(
        names(&reply),
        ["clear", "dream", "remember", "stop", "workspace"]
    );
}
