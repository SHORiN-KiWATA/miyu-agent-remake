//! 监视与推送（施工 8-4，`docs/blueprint/config.md`「协议」的订阅配置的推送、`config.changed`，「怎么走」第七、八条）：
//! 手改推 `config.changed`（`via: file`）、记日志；核心自己写的不重推；改坏了推问题、改好了推空的；删了这一层变空；
//! 订阅、取消订阅、参数不对、掉队推 `resync`；`config.set` 发的那个连接先见推送、后见回应；改了 `ui.language` 连接下一句
//! 就照新的说；手改信任的记录记 `trust.changed`。
//!
//! 等推送一律有上限（十秒），不靠固定的歇一会儿赌时序。

mod support;

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::{Play, Script};
use miyu_store::config_file::version;
use miyu_store::watch::Watch;

use support::*;

/// 个人设置、系统配置在数据根里的位置。
const PERSONAL: &str = "home/alice/settings.toml";
const SYSTEM: &str = "system/config.toml";

/// 照磁盘上的配置起来、开始监视的核心：alice 的家目录建好（监视要看它）。
fn watched(home: &Home, script: &Script) -> (Arc<Core>, Watch) {
    home.root.prepare_home(&alice()).expect("建得了家目录");
    let core = home.core_configured(script, None, &[]);
    let watching = core.watch_config().expect("监视得了");
    (core, watching)
}

/// 连上、握手、订阅配置的推送。
async fn subscribed(core: &Arc<Core>) -> Client {
    let mut client = Client::connect(Arc::clone(core));
    client.hello().await;
    let reply = client
        .call("s1", "subscribe", json!({"stream": "config"}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    client
}

/// 像编辑器那样存：先写旁边的新文件，再改名盖上。
fn save(home: &Home, relative: &str, text: &str) {
    let path = home.root.path().join(relative);
    let temp = path.with_file_name(".settings.toml.swp");
    std::fs::write(&temp, text).expect("写得进");
    std::fs::rename(&temp, &path).expect("改得了名");
}

/// 读下一条，要是 `config.changed`。
async fn pushed(client: &mut Client) -> Value {
    let next = client.next().await.expect("十秒内推了");
    assert_eq!(next["method"], "config.changed", "{next}");
    next["params"].clone()
}

/// 一直问 `config.get`，直到个人设置的版本是 `text` 的（监视看到了、换上了），交回这中间推来的。
async fn until_personal(client: &mut Client, text: &str) -> Vec<Value> {
    let wanted = json!(version(text.as_bytes()));
    let mut pushes = Vec::new();
    for n in 0..1000 {
        let id = format!("v{n}");
        let request = json!({"jsonrpc": "2.0", "id": id, "method": "config.get", "params": {}});
        client.line(&request.to_string()).await;
        let (before, reply) = client.until_reply(&id).await;
        pushes.extend(before);
        if reply["result"]["files"]["personal"]["version"] == wanted {
            return pushes;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("十秒内没换上");
}

/// 一份日志里的每一条。
fn journal(home: &Home, relative: &str) -> Vec<Value> {
    std::fs::read_to_string(home.root.path().join(relative))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("一行一条 JSON"))
        .collect()
}

#[tokio::test]
async fn a_hand_edit_is_pushed_and_logged() {
    let home = Home::new();
    let (core, _watching) = watched(&home, &Script::new([]));
    let mut client = subscribed(&core).await;
    let text = "[ui]\nlanguage = \"en\"\n";
    save(&home, PERSONAL, text);
    assert_eq!(
        pushed(&mut client).await,
        json!({
            "keys": {"ui.language": {
                "applies": "now",
                "effective": "en",
                "origin": {"file": PERSONAL, "layer": "personal", "line": 2},
                "value": "en",
            }},
            "layer": "personal",
            "problems": [],
            "version": version(text.as_bytes()),
            "via": "file",
        }),
        "手改的不带 by"
    );
    let logged = journal(&home, "home/alice/journal.jsonl");
    assert_eq!(logged.len(), 1, "{logged:?}");
    assert_eq!(logged[0]["by"], json!({"kind": "kernel"}));
    assert!(logged[0].get("cause").is_none(), "手改的没有 cause");
    assert_eq!(
        logged[0]["body"],
        json!({"layer": "personal", "file": PERSONAL, "via": "file",
               "changes": [{"key": "ui.language", "new": "en"}]})
    );
    let got = client
        .call("g1", "config.get", json!({"keys": ["ui.language"]}))
        .await;
    assert_eq!(
        got["result"]["items"]["ui.language"]["value"], "en",
        "换上了"
    );
}

#[tokio::test]
async fn the_cores_own_write_is_pushed_once_and_before_the_reply() {
    let home = Home::new();
    let (core, _watching) = watched(&home, &Script::new([]));
    let mut client = subscribed(&core).await;
    let request = json!({"jsonrpc": "2.0", "id": "c1", "method": "config.set",
        "params": {"layer": "personal", "changes": [{"key": "ui.language", "input": "en"}]}});
    client.line(&request.to_string()).await;
    let (before, reply) = client.until_reply("c1").await;
    assert_eq!(before.len(), 1, "先见推送、后见回应：{before:?}");
    let push = &before[0]["params"];
    assert_eq!(push["via"], "set");
    assert_eq!(push["by"], json!({"kind": "person", "account": "alice"}));
    assert_eq!(push["keys"], reply["result"]["keys"], "推的和回应的一样");
    assert_eq!(push["version"], reply["result"]["version"]);
    // 再手改另一份：监视照先后办，核心自己写的那一份要是重推了，会排在它前面。
    save(&home, SYSTEM, "[log]\nlevel = \"debug\"\n");
    let next = pushed(&mut client).await;
    assert_eq!(next["layer"], "system", "自己写的不重推：{next}");
    assert_eq!(next["via"], "file");
    assert_eq!(
        journal(&home, "home/alice/journal.jsonl").len(),
        1,
        "只记了 set 那一条"
    );
}

#[tokio::test]
async fn a_broken_file_pushes_its_problems_and_keeps_the_last_good_values() {
    let home = Home::new();
    let (core, _watching) = watched(&home, &Script::new([]));
    let mut client = subscribed(&core).await;
    save(&home, PERSONAL, "[ui]\nlanguage = \"en\"\n");
    pushed(&mut client).await;
    save(&home, PERSONAL, "[ui]\nlanguage = \"en\n");
    let broken = pushed(&mut client).await;
    assert_eq!(broken["keys"], json!({}), "项照上一次读好的，没变");
    let problems = broken["problems"].as_array().expect("是数组");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0]["code"], "syntax");
    assert_eq!(problems[0]["file"], PERSONAL);
    assert_eq!(problems[0]["using"], json!({"from": "last_good"}));
    let got = client
        .call("g1", "config.get", json!({"keys": ["ui.language"]}))
        .await;
    assert_eq!(got["result"]["items"]["ui.language"]["value"], "en");
    assert_eq!(
        got["result"]["problems"][0]["using"],
        json!({"from": "last_good"})
    );
    save(&home, PERSONAL, "[ui]\nlanguage = \"en\"\n# 好了\n");
    let fixed = pushed(&mut client).await;
    assert_eq!(fixed["keys"], json!({}));
    assert_eq!(fixed["problems"], json!([]), "改好了推一条空的，头收起报错");
}

#[tokio::test]
async fn a_comment_only_edit_is_not_pushed_and_a_deleted_file_empties_its_layer() {
    let home = Home::new();
    let (core, _watching) = watched(&home, &Script::new([]));
    let mut client = subscribed(&core).await;
    save(&home, PERSONAL, "[ui]\nlanguage = \"en\"\n");
    pushed(&mut client).await;
    let comment = "# 只加一行注释\n[ui]\nlanguage = \"en\"\n";
    save(&home, PERSONAL, comment);
    assert_eq!(
        until_personal(&mut client, comment).await,
        Vec::<Value>::new(),
        "只动注释的不推"
    );
    std::fs::remove_file(home.root.path().join(PERSONAL)).expect("删得掉");
    let deleted = pushed(&mut client).await;
    assert_eq!(
        deleted,
        json!({
            "keys": {"ui.language": {
                "applies": "now",
                "effective": "auto",
                "origin": {"layer": "default"},
            }},
            "layer": "personal",
            "problems": [],
            "version": null,
            "via": "file",
        }),
        "删掉的推了"
    );
    assert_eq!(journal(&home, "home/alice/journal.jsonl").len(), 2);
}

#[tokio::test]
async fn subscribing_takes_no_session_and_unsubscribing_stops_the_pushes() {
    let home = Home::new();
    let (core, _watching) = watched(&home, &Script::new([]));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    for (id, params) in [
        ("b1", json!({"stream": "config", "session": "x"})),
        ("b2", json!({"stream": "config", "after": 3})),
        ("b3", json!({"stream": "logs"})),
        ("b4", json!({"stream": "events"})),
    ] {
        let reply = client.call(id, "subscribe", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    }
    let reply = client
        .call("s1", "subscribe", json!({"stream": "config"}))
        .await;
    assert_eq!(reply["result"], json!({}));
    let reply = client
        .call("u1", "unsubscribe", json!({"stream": "config"}))
        .await;
    assert_eq!(reply["result"], json!({}));
    let text = "[ui]\nlanguage = \"en\"\n";
    save(&home, PERSONAL, text);
    assert_eq!(
        until_personal(&mut client, text).await,
        Vec::<Value>::new(),
        "取消了订阅，不推"
    );
    let reply = client
        .call("s2", "subscribe", json!({"stream": "config"}))
        .await;
    assert_eq!(reply["result"], json!({}));
    save(&home, PERSONAL, "[ui]\nlanguage = \"zh\"\n");
    let again = pushed(&mut client).await;
    assert_eq!(
        again["keys"]["ui.language"]["value"], "zh",
        "重新订阅了照样推"
    );
}

#[tokio::test]
async fn a_slow_reader_gets_a_resync_and_every_reply() {
    let home = Home::new();
    // 几千段增量把这个连接的写队列堵上：配置的推送就放不进去，掉队。
    let script = Script::new([Play::Floods(5000)]);
    let core = home.core_configured(&script, None, &[]);
    let mut slow = Client::connect(Arc::clone(&core));
    slow.hello().await;
    let session = slow.create("c1", "~").await;
    slow.subscribe("c2", &session).await;
    slow.call("c3", "subscribe", json!({"stream": "config"}))
        .await;
    let send = json!({"jsonrpc": "2.0", "id": "c4", "method": "session.send",
        "params": {"session": session, "text": "hi"}});
    slow.line(&send.to_string()).await;
    home.until_turns(&session, 1).await;
    let mut other = Client::connect(Arc::clone(&core));
    other.hello().await;
    for n in 0..24 {
        let language = if n % 2 == 0 { "en" } else { "zh" };
        let reply = other
            .call(
                &format!("o{n}"),
                "config.set",
                json!({"layer": "personal", "changes": [{"key": "ui.language", "input": language}]}),
            )
            .await;
        assert!(reply["result"].is_object(), "{reply}");
    }
    let (mut resynced, mut after, mut replies) = (false, 0, Vec::new());
    while let Some(next) = slow.next_within(Duration::from_millis(500)).await {
        match next["method"].as_str() {
            Some("resync") if next["params"]["stream"] == "config" => {
                assert_eq!(next["params"], json!({"stream": "config"}));
                resynced = true;
            }
            Some("config.changed") if resynced => after += 1,
            Some(_) => {}
            None => replies.push(next["id"].clone()),
        }
    }
    assert!(resynced, "读得慢的收到一条配置的 resync");
    assert_eq!(after, 0, "掉了队，这个订阅停了");
    assert_eq!(replies, [json!("c4")], "回应一条都不丢");
}

#[tokio::test]
async fn a_changed_language_is_spoken_on_the_next_request() {
    let home = Home::new();
    let (core, _watching) = watched(&home, &Script::new([]));
    let mut client = subscribed(&core).await;
    let unknown = json!({"keys": ["ui.langauge"]});
    let before = client.call("g1", "config.get", unknown.clone()).await;
    assert_eq!(before["error"]["message"], "没有这一项配置。", "{before}");
    save(&home, PERSONAL, "[ui]\nlanguage = \"en\"\n");
    pushed(&mut client).await;
    let after = client.call("g2", "config.get", unknown).await;
    assert_eq!(
        after["error"]["message"], "There is no such setting.",
        "{after}"
    );
    let schema = client
        .call("k1", "config.schema", json!({"keys": ["ui.language"]}))
        .await;
    assert_eq!(
        schema["result"]["items"][0]["name"], "Interface language",
        "{schema}"
    );
}

#[tokio::test]
async fn a_hand_edited_trust_record_is_logged_and_used() {
    let home = Home::new();
    let repo = home.work.join("app");
    std::fs::create_dir_all(repo.join(".git")).expect("建得了");
    std::fs::create_dir_all(repo.join(".miyu")).expect("建得了");
    let project = "[permission]\nstart_read_only = true\n";
    std::fs::write(repo.join(".miyu").join("config.toml"), project).expect("写得进");
    let (core, _watching) = watched(&home, &Script::new([]));
    let mut client = subscribed(&core).await;
    let cwd = repo.to_string_lossy().into_owned();
    let record = format!(
        "[[project]]\npath = {}\nversion = \"{}\"\ntrusted = true\n",
        serde_json::to_string(&cwd).expect("写得成"),
        version(project.as_bytes())
    );
    std::fs::write(home.root.path().join("home/alice/trust.toml"), record).expect("写得进");
    let get = json!({"keys": ["permission.start_read_only"], "cwd": cwd});
    let mut n = 0;
    loop {
        n += 1;
        let got = client
            .call(&format!("g{n}"), "config.get", get.clone())
            .await;
        if got["result"]["files"]["project"]["trusted"] == json!(true) {
            assert_eq!(
                got["result"]["items"]["permission.start_read_only"]["value"],
                true
            );
            break;
        }
        assert!(n < 1000, "十秒内认了手改的信任：{got}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let logged = journal(&home, "home/alice/journal.jsonl");
    assert_eq!(logged.len(), 1, "{logged:?}");
    assert_eq!(logged[0]["kind"], "trust.changed");
    assert_eq!(logged[0]["by"], json!({"kind": "kernel"}));
    assert_eq!(logged[0]["body"]["trusted"], true);
    assert_eq!(logged[0]["body"]["via"], "file");
    assert_eq!(
        client.next_within(Duration::from_millis(300)).await,
        None,
        "项目配置的信任不推"
    );
}

/// 说一句，等第 `n` 轮在磁盘上说完。
async fn turn(home: &Home, client: &mut Client, session: &str, n: usize) {
    let reply = client.say(&format!("s{n}"), session, "hi").await;
    assert!(reply["result"].is_object(), "{reply}");
    home.until_turns(session, n).await;
}

/// 回合开始时的配置经端点的配置服务取（第八条第 3 条）：每一轮照会话的目录重读项目配置，信任着的才算；回合之间
/// `config.set` 改的，下一轮用上。
#[tokio::test]
async fn every_turn_takes_the_config_of_that_moment() {
    let home = Home::new();
    let repo = home.work.join("app");
    std::fs::create_dir_all(repo.join(".git")).expect("建得了");
    std::fs::create_dir_all(repo.join(".miyu")).expect("建得了");
    let project = "[permission]\nstart_read_only = true\n";
    let config = repo.join(".miyu").join("config.toml");
    std::fs::write(&config, project).expect("写得进");
    let script = Script::new([Play::Says("好。"), Play::Says("好。"), Play::Says("好。")]);
    let core = home.core_configured(&script, None, &[]);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = repo.to_string_lossy().into_owned();
    let trusted = client
        .call(
            "t1",
            "config.trust",
            json!({"cwd": cwd, "version": version(project.as_bytes()), "trust": true}),
        )
        .await;
    assert!(trusted["result"].is_object(), "{trusted}");
    let session = client.create("c1", &cwd).await;
    turn(&home, &mut client, &session, 1).await;
    // 内容变了，信任跟着失效：下一轮不算。
    std::fs::write(&config, format!("# 改过\n{project}")).expect("写得进");
    turn(&home, &mut client, &session, 2).await;
    let set = json!({"layer": "personal", "changes": [{"key": "permission.start_read_only", "input": "true"}]});
    let reply = client.call("p1", "config.set", set).await;
    assert!(reply["result"].is_object(), "{reply}");
    turn(&home, &mut client, &session, 3).await;
    let read_only: Vec<(Value, Value)> = script
        .configs()
        .iter()
        .map(|config| {
            let (value, origin) = config
                .resolved
                .get("permission.start_read_only")
                .expect("有这一项");
            (value.json(), json!(origin.layer_name()))
        })
        .collect();
    assert_eq!(
        read_only,
        [
            (json!(true), json!("project")),
            (json!(false), json!("default")),
            (json!(true), json!("personal")),
        ]
    );
}

/// 字节和上一次读的一样的（核心自己写的也走这里），什么都不做：不换、不交给会话和核心（第七条第 3 条）。
#[tokio::test]
async fn the_same_bytes_are_left_alone() {
    let home = Home::new();
    home.root.prepare_home(&alice()).expect("建得了家目录");
    home.write(PERSONAL, "[ui]\nlanguage = \"en\"\n");
    let core = home.core_configured(&Script::new([]), None, &[]);
    let mut now = core.config_now();
    now.borrow_and_update();
    let path = home.root.path().join(PERSONAL);
    core.config_file_changed(&path);
    assert!(!now.has_changed().expect("还在"), "一样的不换");
    home.write(PERSONAL, "# 注释\n[ui]\nlanguage = \"en\"\n");
    core.config_file_changed(&path);
    assert!(
        now.has_changed().expect("还在"),
        "只动注释的换上（版本跟着换）"
    );
}
