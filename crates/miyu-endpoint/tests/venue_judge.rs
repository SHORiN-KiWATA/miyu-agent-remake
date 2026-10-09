//! 判官看的群聊记录（施工 O-24，`docs/construction/O-24-判官看的群聊记录.md`）：`venue.records` 照会话日志渲染要判的那一条和它之前
//! 的几条，写法和她看到的一行一样；`msg` 不是场所消息的、`count` 不对的 `bad_params`；没有这个会话的找不到。私聊的照核心这时
//! 的时区（主人的私聊，这里只看形状）。

use serde_json::json;

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

#[tokio::test]
async fn records_are_rendered_like_her_lines() {
    let home = Home::new();
    let mut client = Client::connect(bound_core(&home, &Script::new([]), Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let mut seqs = Vec::new();
    for (n, (text, msg)) in [("早", "8801"), ("判我", "8802")].into_iter().enumerate() {
        let reply = client
            .call(
                &format!("o{n}"),
                "session.send",
                json!({"session": session, "text": text, "as": {"external": "qq:10001"},
                       "venue": {"msg": msg, "name": "主人", "ambient": true}}),
            )
            .await;
        seqs.push(
            reply["result"]["events"][0]
                .as_u64()
                .unwrap_or_else(|| panic!("{reply}")),
        );
    }
    let reply = client
        .call(
            "r1",
            "venue.records",
            json!({"session": session, "msg": seqs[1], "count": 20}),
        )
        .await;
    let records = reply["result"]["records"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"));
    let current = reply["result"]["current"].as_str().expect("有这一条");
    assert!(
        records.ends_with("] 主人 (owner) [msg=8801]: 早\n") && records.starts_with('['),
        "{records}"
    );
    assert!(
        current.ends_with("] 主人 (owner) [msg=8802]: 判我"),
        "{current}"
    );
    for (n, params) in [
        json!({"session": session, "msg": 1, "count": 20}),
        json!({"session": session, "msg": seqs[1], "count": 0}),
        json!({"session": session, "msg": seqs[1], "count": 101}),
        json!({"session": session, "msg": 0, "count": 20}),
        json!({"session": session, "msg": seqs[1]}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("b{n}"), "venue.records", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
    }
    let reply = client
        .call(
            "n1",
            "venue.records",
            json!({"session": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91", "msg": 2, "count": 20}),
        )
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
}
