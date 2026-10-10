//! 视图流（施工 9-8 下，`docs/blueprint/view.md`「视图流」）：订阅的回应带最新一页的条目，排在所有推送前面；之后的推送
//! 拼到这一页上，最后和 `view.page {view: true}` 一样；`turn.started`、`turn.ended` 照原样另推；连接的语言改了，之后的
//! 条目照新的字；带 `after` 的不收；退订了不再推。

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 头手里的条目照一条推送改：照图纸「视图流」的表。
fn apply(entries: &mut Vec<Value>, push: &Value) {
    let params = &push["params"];
    let at = |entries: &Vec<Value>, id: &Value| entries.iter().position(|entry| &entry["id"] == id);
    let place = |entries: &mut Vec<Value>, entry: Value, after: &Value| {
        let index = match after {
            Value::Null => 0,
            id => at(entries, id).map_or(entries.len(), |index| index + 1),
        };
        entries.insert(index, entry);
    };
    match push["method"].as_str() {
        Some("view.add") => place(entries, params["entry"].clone(), &params["after"]),
        Some("view.update") => {
            let entry = params["entry"].clone();
            let index = at(entries, &entry["id"]).unwrap_or_else(|| panic!("换的那一条在：{push}"));
            match params.get("after") {
                Some(after) => {
                    entries.remove(index);
                    place(entries, entry, after);
                }
                None => entries[index] = entry,
            }
        }
        Some("view.append") => {
            let index =
                at(entries, &params["id"]).unwrap_or_else(|| panic!("接字的那一条在：{push}"));
            let entry = &mut entries[index];
            let key = match entry["kind"].as_str() {
                Some("tool") => "args",
                _ => "text",
            };
            let joined = format!(
                "{}{}",
                entry[key].as_str().unwrap_or_default(),
                params["text"].as_str().unwrap_or_default()
            );
            entry[key] = json!(joined);
        }
        Some("view.hidden") => {
            for id in params["ids"].as_array().into_iter().flatten() {
                if let Some(index) = at(entries, id)
                    && let Some(entry) = entries[index].as_object_mut()
                {
                    match params["hidden"] == json!(true) {
                        true => drop(entry.insert("hidden".to_string(), json!(true))),
                        false => drop(entry.remove("hidden")),
                    }
                }
            }
        }
        Some("view.remove") => {
            if let Some(index) = at(entries, &params["id"]) {
                entries.remove(index);
            }
        }
        _ => {}
    }
}

/// 发一句话，读到它的回应和这一轮结束都到了：交回之间推来的。回合跑得快的，`turn.ended` 可能排在回应前面。
async fn say_through(client: &mut Client, id: &str, session: &str, text: &str) -> Vec<Value> {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": "session.send",
        "params": {"session": session, "text": text}});
    client.line(&request.to_string()).await;
    let mut pushed = Vec::new();
    let (mut replied, mut ended) = (false, false);
    while !(replied && ended) {
        let next = client.next().await.expect("没断开");
        if next["id"] == json!(id) {
            replied = true;
            continue;
        }
        ended |= next["params"]["session"] == json!(session)
            && next["params"]["event"]["kind"] == json!("turn.ended");
        pushed.push(next);
    }
    pushed
}

#[tokio::test]
async fn the_stream_builds_on_the_page_and_ends_like_it() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "a").await;
    home.until_turns(&session, 1).await;

    let subscribed = client
        .call(
            "v1",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    assert_eq!(subscribed["id"], "v1", "回应排在推送前面：{subscribed}");
    let result = &subscribed["result"];
    assert!(result["limits"].is_object(), "和事件流一样带限额：{result}");
    let mut entries = result["entries"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("带最新一页：{subscribed}"));
    assert_eq!(
        entries
            .iter()
            .map(|e| e["kind"].clone())
            .collect::<Vec<_>>(),
        [json!("user"), json!("reply"), json!("end")]
    );

    let pushed = say_through(&mut client, "s2", &session, "b").await;
    assert!(
        pushed
            .iter()
            .any(|push| push["method"] == "event"
                && push["params"]["event"]["kind"] == "turn.started"),
        "turn.started 照原样另推：{pushed:#?}"
    );
    assert!(
        pushed.iter().any(|push| push["method"] == "view.add"),
        "{pushed:#?}"
    );
    for push in &pushed {
        apply(&mut entries, push);
    }
    let page = client
        .call("p1", "view.page", json!({"session": session, "view": true}))
        .await;
    assert_eq!(
        json!(entries),
        page["result"]["entries"],
        "视图流拼出来的和翻页一样"
    );
}

#[tokio::test]
async fn a_new_language_draws_the_next_entries() {
    let home = Home::new();
    std::fs::write(home.work.join("a.txt"), "hello\n").expect("写得进工作区");
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::Says("好。"),
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client
        .call(
            "v1",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    let set = client
        .call(
            "g1",
            "config.set",
            json!({"layer": "personal", "changes": [{"key": "ui.language", "value": "en"}]}),
        )
        .await;
    assert!(set.get("result").is_some(), "{set}");
    let pushed = say_through(&mut client, "s1", &session, "read it").await;
    let names: Vec<&Value> = pushed
        .iter()
        .filter(|push| push["params"]["entry"]["kind"] == "tool")
        .map(|push| &push["params"]["entry"]["title"]["name"])
        .collect();
    assert!(!names.is_empty(), "{pushed:#?}");
    assert!(
        names.iter().all(|name| name.as_str() != Some("读取")),
        "改成英文以后照英文：{names:?}"
    );
}

#[tokio::test]
async fn after_is_refused_and_unsubscribing_stops_it() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let refused = client
        .call(
            "v1",
            "subscribe",
            json!({"session": session, "stream": "view", "after": 0}),
        )
        .await;
    assert_eq!(reason(&refused), Some("bad_params"), "{refused}");
    client
        .call(
            "v2",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    let left = client
        .call(
            "u1",
            "unsubscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    assert_eq!(left["result"], json!({}), "{left}");
    client.say("s1", &session, "a").await;
    home.until_turns(&session, 1).await;
    assert!(
        client
            .next_within(Duration::from_millis(300))
            .await
            .is_none(),
        "退订了不再推"
    );
}
