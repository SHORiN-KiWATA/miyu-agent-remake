//! 人经协议清空记忆（施工 R-3 补，`docs/blueprint/memory.md` 第二条第 5 款）：真核心、真记忆日志，数据根在临时目录。清掉从
//! 一个会话来的、清掉整间；会话那一间清会话的就是整间。

use serde_json::json;

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::memories::{connected, pairs, texts, with_persona};
use crate::support::*;

/// 她经工具记的、人记的混在一起：清掉从一个会话来的只清出处全在它里面的，人记的不动；清掉关于我的是整间。
#[tokio::test]
async fn clearing_takes_one_sessions_memories_or_the_whole_room() {
    let home = Home::new();
    let remember = |text: &str| {
        Play::calls(&[(
            "remember",
            &json!({"class": "user", "text": text}).to_string(),
        )])
    };
    let script = Script::new([
        remember("用户养了一只猫"),
        Play::Says("记住了。"),
        remember("用户爱喝茶"),
        Play::Says("记住了。"),
    ]);
    let resources = default_resources();
    let mut tools = miyu_basesystem::tools(&resources).expect("读得出");
    tools.extend(miyu_memory::tools(&resources).expect("读得出"));
    let catalog = Catalog::new(tools).expect("合写法");
    let mut client = Client::connect(with_persona(&home, &script, catalog));
    client.hello().await;
    let cat = client.create("c1", "~").await;
    client.say("c2", &cat, "我养了一只猫").await;
    home.until_turns(&cat, 1).await;
    let tea = client.create("c3", "~").await;
    client.say("c4", &tea, "我爱喝茶").await;
    home.until_turns(&tea, 1).await;
    client
        .call(
            "c5",
            "memory.remember",
            json!({"class": "user", "text": "用户住在上海"}),
        )
        .await;
    let listed = client.call("l1", "memory.list", json!({})).await;
    assert_eq!(
        texts(&listed),
        pairs(&[
            ("m3", "用户住在上海"),
            ("m2", "用户爱喝茶"),
            ("m1", "用户养了一只猫")
        ])
    );
    assert_eq!(listed["result"]["memories"][1]["by"], "tool", "她记的");
    assert_eq!(
        listed["result"]["memories"][1]["sources"][0]["session"],
        tea.as_str()
    );
    let from = client.call("l2", "memory.list", json!({"from": tea})).await;
    assert_eq!(texts(&from), pairs(&[("m2", "用户爱喝茶")]));

    let reply = client
        .call(
            "c6",
            "memory.forget",
            json!({"clear": "session", "session": cat}),
        )
        .await;
    assert_eq!(reply["result"], json!({"cleared": 1}), "{reply}");
    let all = client
        .call("l3", "memory.list", json!({"forgotten": true}))
        .await;
    assert_eq!(
        texts(&all),
        pairs(&[("m3", "用户住在上海"), ("m2", "用户爱喝茶")]),
        "清掉的作废的里也没有"
    );
    let reply = client
        .call("c7", "memory.forget", json!({"clear": "session"}))
        .await;
    assert_eq!(
        reason(&reply),
        Some("bad_params"),
        "清会话的要写是哪个：{reply}"
    );
    let reply = client
        .call("c8", "memory.forget", json!({"clear": "me"}))
        .await;
    assert_eq!(reply["result"], json!({"cleared": 2}), "{reply}");
    let all = client
        .call("l4", "memory.list", json!({"forgotten": true}))
        .await;
    assert!(texts(&all).is_empty(), "{all}");
    let found = client
        .call(
            "s1",
            "memory.search",
            json!({"query": "喝茶", "forgotten": true}),
        )
        .await;
    assert!(texts(&found).is_empty(), "{found}");
    let reply = client
        .call("c9", "memory.update", json!({"id": "m3", "text": "x"}))
        .await;
    assert_eq!(
        reason(&reply),
        Some("memory_not_current"),
        "清掉的不算了：{reply}"
    );
}

/// 会话那一间：清掉这个会话的就是整间。
#[tokio::test]
async fn clearing_a_session_scope_session_clears_its_room() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let created = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": "~", "memory": "session"}),
        )
        .await;
    let own = created["result"]["session"]
        .as_str()
        .expect("造了")
        .to_string();
    for (id, text) in [("c2", "一"), ("c3", "二")] {
        client
            .call(
                id,
                "memory.remember",
                json!({"class": "user", "text": text, "session": own}),
            )
            .await;
    }
    let reply = client
        .call(
            "c4",
            "memory.forget",
            json!({"clear": "session", "session": own}),
        )
        .await;
    assert_eq!(reply["result"], json!({"cleared": 2}), "{reply}");
}
