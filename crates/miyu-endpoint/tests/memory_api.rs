//! 人经协议碰记忆（施工 R-3 补，`docs/blueprint/memory.md`「协议」、第二条第 5 款）：真核心、真记忆日志，数据根在临时目录。
//! 五个方法各走一遍；照人格、照会话、照默认找哪一间；写错的几种；同一个命令编号只算一次。清空在 `memory_clear.rs`。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::memories::{connected, pairs, texts};
use crate::support::*;

#[tokio::test]
async fn remember_list_search_update_and_forget() {
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
    let reply = client
        .call(
            "c2",
            "memory.remember",
            json!({"class": "feedback", "text": "回答要短"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m2"}), "{reply}");

    let listed = client.call("l1", "memory.list", json!({})).await;
    assert_eq!(
        texts(&listed),
        pairs(&[("m2", "回答要短"), ("m1", "用户用 N 卡")]),
        "新的在前"
    );
    let first = &listed["result"]["memories"][1];
    assert_eq!(first["class"], "user");
    assert_eq!(first["by"], "person", "人记的");
    assert_eq!(first["sources"], json!([]));
    assert_eq!(first["retired"], Value::Null);
    assert!(
        first["at"].as_str().is_some_and(|at| at.ends_with('Z')),
        "{first}"
    );
    let users = client
        .call("l2", "memory.list", json!({"class": "user"}))
        .await;
    assert_eq!(texts(&users), pairs(&[("m1", "用户用 N 卡")]));
    let limited = client.call("l3", "memory.list", json!({"limit": 1})).await;
    assert_eq!(texts(&limited), pairs(&[("m2", "回答要短")]));
    let found = client
        .call("s1", "memory.search", json!({"query": "N卡"}))
        .await;
    assert_eq!(texts(&found), pairs(&[("m1", "用户用 N 卡")]));

    let reply = client
        .call(
            "c3",
            "memory.update",
            json!({"id": "m1", "text": "用户用 RTX 4060"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m3"}), "{reply}");
    let listed = client.call("l4", "memory.list", json!({})).await;
    assert_eq!(
        texts(&listed),
        pairs(&[("m3", "用户用 RTX 4060"), ("m2", "回答要短")]),
        "改掉的不出来"
    );
    assert_eq!(listed["result"]["memories"][0]["class"], "user", "类照旧的");
    let again = client
        .call("c4", "memory.update", json!({"id": "m1", "text": "x"}))
        .await;
    assert_eq!(reason(&again), Some("memory_not_current"), "{again}");

    let reply = client
        .call("c5", "memory.forget", json!({"id": "m2", "why": "不对"}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let listed = client.call("l5", "memory.list", json!({})).await;
    assert_eq!(texts(&listed), pairs(&[("m3", "用户用 RTX 4060")]));
    let all = client
        .call("l6", "memory.list", json!({"forgotten": true}))
        .await;
    assert_eq!(
        texts(&all),
        pairs(&[("m3", "用户用 RTX 4060"), ("m2", "回答要短")])
    );
    assert_eq!(all["result"]["memories"][1]["retired"], "不对");
    let found = client
        .call(
            "s2",
            "memory.search",
            json!({"query": "回答", "forgotten": true}),
        )
        .await;
    assert_eq!(texts(&found), pairs(&[("m2", "回答要短")]));
    for (id, params, why) in [
        ("f1", json!({"id": "m2"}), "memory_not_current"),
        ("f2", json!({"id": "m99"}), "unknown_memory"),
        ("f3", json!({"id": "m3", "clear": "me"}), "bad_params"),
        ("f4", json!({}), "bad_params"),
        ("f5", json!({"clear": "everything"}), "bad_params"),
        ("f6", json!({"id": "3"}), "bad_params"),
    ] {
        let reply = client.call(id, "memory.forget", params.clone()).await;
        assert_eq!(reason(&reply), Some(why), "{params}：{reply}");
    }
    let reply = client
        .call("c6", "memory.forget", json!({"id": "m3"}))
        .await;
    assert_eq!(reply["result"], json!({}), "why 可以不写");
    let reply = client
        .call(
            "c7",
            "memory.remember",
            json!({"class": "episode", "text": "一起爬了山"}),
        )
        .await;
    let id = reply["result"]["id"].as_str().expect("记下了").to_string();
    let reply = client
        .call(
            "c8",
            "memory.update",
            json!({"id": id, "text": "一起爬了泰山"}),
        )
        .await;
    assert!(reply["result"]["id"].is_string(), "{reply}");
    let listed = client.call("l7", "memory.list", json!({"limit": 1})).await;
    assert_eq!(
        listed["result"]["memories"][0]["class"], "episode",
        "改了类照旧的：{listed}"
    );
}

#[tokio::test]
async fn wrong_requests_are_refused_and_nothing_is_written() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let long = "长".repeat(121);
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": long}),
        )
        .await;
    assert_eq!(reason(&reply), Some("memory_too_long"), "{reply}");
    assert_eq!(reply["error"]["data"]["limit"], 120);
    assert_eq!(reply["error"]["data"]["chars"], 121);
    let reply = client
        .call(
            "c2",
            "memory.remember",
            json!({"class": "user", "text": "长".repeat(120)}),
        )
        .await;
    assert_eq!(
        reply["result"],
        json!({"id": "m1"}),
        "120 字记得下：{reply}"
    );
    for (id, params) in [
        ("c3", json!({"class": "nope", "text": "x"})),
        ("c4", json!({"class": "user", "text": "  "})),
        ("c5", json!({"class": "user"})),
        (
            "c6",
            json!({"class": "user", "text": "x", "persona": "engineer", "session": "0192f3a0-1111-7abc-8def-001122334455"}),
        ),
        (
            "c7",
            json!({"class": "user", "text": "x", "as": {"platform": "qq", "id": "1"}}),
        ),
        (
            "c8",
            json!({"class": "user", "text": "x", "persona": "../x"}),
        ),
    ] {
        let reply = client.call(id, "memory.remember", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    let reply = client
        .call(
            "c9",
            "memory.remember",
            json!({"class": "user", "text": "x", "persona": "nobody"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("unknown_persona"), "{reply}");
    let reply = client
        .call(
            "c10",
            "memory.list",
            json!({"session": "0192f3a0-1111-7abc-8def-001122334455"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
    let reply = client
        .call("c11", "memory.update", json!({"id": "m1", "text": long}))
        .await;
    assert_eq!(reason(&reply), Some("memory_too_long"), "{reply}");
    for (id, method, params) in [
        ("c12", "memory.search", json!({"query": ""})),
        ("c13", "memory.search", json!({"query": "x", "limit": 0})),
        ("c14", "memory.list", json!({"limit": 501})),
        ("c15", "memory.list", json!({"from": "not-a-session"})),
    ] {
        let reply = client.call(id, method, params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    let reply = client
        .call("c16", "memory.list", json!({"limit": 500}))
        .await;
    assert!(reply["result"]["memories"].is_array(), "500 可以：{reply}");
    let listed = client
        .call("l1", "memory.list", json!({"forgotten": true}))
        .await;
    assert_eq!(texts(&listed).len(), 1, "写错的什么都没记");
}

#[tokio::test]
async fn the_room_is_the_personas_the_sessions_or_the_defaults() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "跟着人格的", "persona": "engineer"}),
        )
        .await;
    assert_eq!(reply["result"]["id"], "m1", "{reply}");
    let listed = client.call("l1", "memory.list", json!({})).await;
    assert_eq!(
        texts(&listed),
        pairs(&[("m1", "跟着人格的")]),
        "默认就是软件工程师"
    );

    let created = client
        .call(
            "c2",
            "session.create",
            json!({"cwd": "~", "memory": "session"}),
        )
        .await;
    let own = created["result"]["session"]
        .as_str()
        .expect("造了")
        .to_string();
    let reply = client
        .call(
            "c3",
            "memory.remember",
            json!({"class": "user", "text": "只在会话里的", "session": own}),
        )
        .await;
    assert_eq!(reply["result"]["id"], "m1", "会话那一间自己编号：{reply}");
    let listed = client
        .call("l2", "memory.list", json!({"session": own}))
        .await;
    assert_eq!(texts(&listed), pairs(&[("m1", "只在会话里的")]));
    let listed = client.call("l3", "memory.list", json!({})).await;
    assert_eq!(
        texts(&listed),
        pairs(&[("m1", "跟着人格的")]),
        "人格那一间没有它"
    );

    let created = client
        .call("c4", "session.create", json!({"cwd": "~"}))
        .await;
    let follows = created["result"]["session"]
        .as_str()
        .expect("造了")
        .to_string();
    let listed = client
        .call("l4", "memory.list", json!({"session": follows}))
        .await;
    assert_eq!(
        texts(&listed),
        pairs(&[("m1", "跟着人格的")]),
        "跟着人格的会话用人格那一间"
    );

    let created = client
        .call("c5", "session.create", json!({"cwd": "~", "memory": "off"}))
        .await;
    let off = created["result"]["session"]
        .as_str()
        .expect("造了")
        .to_string();
    for method in [
        "memory.list",
        "memory.search",
        "memory.remember",
        "memory.dream",
    ] {
        let reply = client
            .call(
                method,
                method,
                json!({"session": off, "query": "x", "class": "user", "text": "x"}),
            )
            .await;
        assert_eq!(
            reason(&reply),
            Some("memory_unavailable"),
            "{method}：{reply}"
        );
    }
}

/// 同一个命令编号再发只算一次（04 第六节第 1 条）：记、改、作废、清空都交回头一次的结果，什么都不多写；核心重启以后照记忆
/// 日志认得。
#[tokio::test]
async fn the_same_command_counts_once_even_after_a_restart() {
    let home = Home::new();
    let mut client = connected(&home, &Script::new([])).await;
    let remember = json!({"class": "user", "text": "一"});
    for _ in 0..2 {
        let reply = client.call("c1", "memory.remember", remember.clone()).await;
        assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    }
    for _ in 0..2 {
        let reply = client
            .call("c2", "memory.update", json!({"id": "m1", "text": "一改"}))
            .await;
        assert_eq!(
            reply["result"],
            json!({"id": "m2"}),
            "改掉的那一条已经不算了也照回：{reply}"
        );
    }
    for _ in 0..2 {
        let reply = client
            .call("c3", "memory.forget", json!({"id": "m2"}))
            .await;
        assert_eq!(reply["result"], json!({}), "{reply}");
    }
    let reply = client
        .call(
            "c4",
            "memory.remember",
            json!({"class": "user", "text": "二"}),
        )
        .await;
    assert_eq!(
        reply["result"],
        json!({"id": "m4"}),
        "日志里只多了三行：{reply}"
    );
    for _ in 0..2 {
        let reply = client
            .call("c5", "memory.forget", json!({"clear": "me"}))
            .await;
        assert_eq!(
            reply["result"],
            json!({"cleared": 2}),
            "作废的和还算数的，不算改掉的旧版本：{reply}"
        );
    }
    drop(client);

    let mut client = connected(&home, &Script::new([])).await;
    let reply = client.call("c1", "memory.remember", remember).await;
    assert_eq!(
        reply["result"],
        json!({"id": "m1"}),
        "重启以后照日志认得：{reply}"
    );
    let reply = client
        .call("c5", "memory.forget", json!({"clear": "me"}))
        .await;
    assert_eq!(reply["result"], json!({"cleared": 2}), "{reply}");
    let reply = client
        .call(
            "c6",
            "memory.remember",
            json!({"class": "user", "text": "三"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m6"}), "{reply}");
}

/// 不带人格记忆不生效（17 L17，施工 P-4 上起出厂不设默认人格）：不写人格、默认人格也没设的 `memory.*` 没有哪一间，不带人格
/// 的会话里 `/remember` 也不记；明着写了人格的照样能用。
#[tokio::test]
async fn without_a_persona_there_is_no_memory() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client.call("l1", "memory.list", json!({})).await;
    assert_eq!(reason(&reply), Some("memory_unavailable"), "{reply}");
    let session = client.create("c1", "~").await;
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/remember 用户养猫"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("memory_unavailable"), "{reply}");
    let reply = client
        .call(
            "c2",
            "memory.remember",
            json!({"class": "user", "text": "用户养猫", "persona": "engineer"}),
        )
        .await;
    assert_eq!(
        reply["result"],
        json!({"id": "m1"}),
        "明着写了人格的照样能记：{reply}"
    );
}
