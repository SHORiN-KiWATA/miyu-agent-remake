//! 新建、改、删预设（施工 P-3 中、P-3 补，`docs/blueprint/presets.md`「改」）：真核心走一遍。只写家目录那一层；不写编号的是
//! 新建、编号由核心起；改出厂的只写改了的项；原来的注释、顺序照原样；有错、`expect` 对不上的整条不收、什么都不写；删掉家目录
//! 那一层回到下面的。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::*;
use miyu_endpoint::Core;
use miyu_session::testkit::Script;

fn core(home: &Home) -> Arc<Core> {
    home.core(&Script::new([]))
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(core(home));
    client.hello().await;
    client
}

/// 家目录那一层的文件，没有的是空的。
fn mine(home: &Home, id: &str) -> Option<String> {
    std::fs::read_to_string(
        home.root
            .path()
            .join(format!("home/alice/presets/{id}.toml")),
    )
    .ok()
}

async fn set(client: &mut Client, id: &str, preset: &str, changes: Value) -> Value {
    client
        .call(
            id,
            "preset.set",
            json!({"preset": preset, "changes": changes}),
        )
        .await
}

/// 回应里软件 `id` 开不开；没有这一个的是没有。
fn on(reply: &Value, id: &str) -> Option<bool> {
    reply["result"]["software"]
        .as_array()?
        .iter()
        .find(|one| one["id"] == id)?["on"]
        .as_bool()
}

#[tokio::test]
async fn a_new_preset_gets_its_id_from_the_core_and_a_session_can_use_it() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let made = client
        .call(
            "s1",
            "preset.set",
            json!({"changes": [
                {"key": "preset.name", "value": "我的"},
                {"key": "software.memory", "value": false},
            ]}),
        )
        .await;
    assert_eq!(made["result"]["preset"], "preset-1", "{made}");
    assert_eq!(made["result"]["name"], "我的");
    assert_eq!(on(&made, "memory"), Some(false));
    assert_eq!(on(&made, "roleplay"), Some(true), "别的照旧全开");
    assert_eq!(made["result"]["remove"], "delete", "自己建的：删了就没了");
    assert_eq!(
        mine(&home, "preset-1").as_deref(),
        Some("[preset]\nname = \"我的\"\n\n[software]\nmemory = false\n")
    );
    let second = client
        .call(
            "s2",
            "preset.set",
            json!({"changes": [{"key": "preset.name", "value": "又一个"}]}),
        )
        .await;
    assert_eq!(second["result"]["preset"], "preset-2", "{second}");
    let nothing = client
        .call(
            "s3",
            "preset.set",
            json!({"changes": [{"key": "software.net", "unset": true}]}),
        )
        .await;
    assert_eq!(
        reason(&nothing),
        Some("bad_params"),
        "新建只删不写：{nothing}"
    );
    let created = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": home.work.to_string_lossy(), "preset": "preset-1"}),
        )
        .await;
    assert!(created["result"]["session"].is_string(), "{created}");
}

#[tokio::test]
async fn changing_a_shipped_one_writes_only_what_changed_and_keeps_the_rest_of_the_file() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let got = set(
        &mut client,
        "s1",
        "dev",
        json!([{"key": "software.memory", "value": true}]),
    )
    .await;
    assert_eq!(on(&got, "memory"), Some(true), "{got}");
    assert_eq!(
        got["result"]["remove"], "restore",
        "改过的出厂：删了回到出厂的"
    );
    assert_eq!(
        mine(&home, "dev").as_deref(),
        Some("[software]\nmemory = true\n")
    );
    let untouched = client
        .call("g0", "preset.get", json!({"preset": "full"}))
        .await;
    assert_eq!(
        untouched["result"]["remove"],
        Value::Null,
        "没改过的出厂没什么可删"
    );

    home.write(
        "home/alice/presets/full.toml",
        "# 我自己的\n[tools]\nshell = false # 先关着\n\n[preset]\nname = { zh = \"全开\" }\n",
    );
    let renamed = set(
        &mut client,
        "s2",
        "full",
        json!([
            {"key": "preset.name", "value": "All"},
            {"key": "tools.trash", "value": false},
        ]),
    )
    .await;
    assert_eq!(renamed["result"]["name"], "All", "{renamed}");
    assert_eq!(
        mine(&home, "full").as_deref(),
        Some(
            "# 我自己的\n[tools]\nshell = false # 先关着\ntrash = false\n\n[preset]\nname = \"All\"\n"
        ),
        "注释、顺序照原样；以前的语言表换成一句字"
    );

    // 删一项：回到下面那一层的。
    let unset = set(
        &mut client,
        "s3",
        "dev",
        json!([{"key": "software.memory", "unset": true}]),
    )
    .await;
    assert_eq!(
        on(&unset, "memory"),
        None,
        "没装、也不再写着的不列：{unset}"
    );
}

#[tokio::test]
async fn conflicts_and_mistakes_write_nothing() {
    let home = Home::new();
    let mut client = connected(&home).await;
    set(
        &mut client,
        "s0",
        "dev",
        json!([{"key": "software.memory", "value": true}]),
    )
    .await;
    for (n, (expect, current)) in [
        (json!({"value": false}), json!({"value": true})),
        (json!({}), json!({"value": true})),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = set(
            &mut client,
            &format!("e{n}"),
            "dev",
            json!([{"key": "software.memory", "value": false, "expect": expect}]),
        )
        .await;
        assert_eq!(reason(&reply), Some("preset_conflict"), "{reply}");
        assert_eq!(reply["error"]["data"]["current"], current);
    }
    let matching = set(
        &mut client,
        "e9",
        "dev",
        json!([{"key": "software.net", "value": false, "expect": {}}]),
    )
    .await;
    assert_eq!(
        on(&matching, "net"),
        Some(false),
        "对得上的照写：{matching}"
    );
    let before = mine(&home, "dev");

    for (n, (changes, problem)) in [
        (
            json!([{"key": "software.memory", "value": "yes"}]),
            "home dev.toml:2: software.memory must be true or false",
        ),
        (
            json!([{"key": "preset.colour", "value": "red"}]),
            "home dev.toml:",
        ),
        (
            json!([{"key": "preset.name", "value": "  "}]),
            "home dev.toml:",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = set(&mut client, &format!("i{n}"), "dev", changes.clone()).await;
        assert_eq!(reason(&reply), Some("preset_invalid"), "{changes}：{reply}");
        let said = reply["error"]["data"]["problem"]
            .as_str()
            .unwrap_or_default();
        assert!(said.starts_with(problem), "{said}");
        let told = reply["error"]["data"]["message"]
            .as_str()
            .unwrap_or_default();
        assert!(
            !told.is_empty() && !told.contains("home"),
            "照连接的语言说一句，不带层：{reply}"
        );
        assert_eq!(mine(&home, "dev"), before, "什么都没写：{changes}");
    }

    for (n, params) in [
        json!({"preset": "dev", "changes": []}),
        json!({"changes": []}),
        json!({"preset": "dev", "changes": [{"key": "software.net", "value": true, "unset": true}]}),
        json!({"preset": "dev", "changes": [{"key": "software.net"}]}),
        json!({"preset": "dev", "changes": [{"key": "software.net", "value": [true]}]}),
        json!({"preset": "dev", "changes": [{"key": "software.net", "unset": false}]}),
        json!({"preset": "dev", "changes": [{"key": "a", "value": 1}, {"key": "a", "value": 2}]}),
        json!({"preset": "dev", "changes": [{"key": "software.net", "value": true, "expect": {"was": 1}}]}),
        json!({"preset": "Not An Id", "changes": [{"key": "software.net", "value": true}]}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("b{n}"), "preset.set", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    assert_eq!(mine(&home, "dev"), before);
}

#[tokio::test]
async fn deleting_your_layer_goes_back_to_what_is_below() {
    let home = Home::new();
    let mut client = connected(&home).await;
    set(
        &mut client,
        "s1",
        "dev",
        json!([{"key": "software.memory", "value": true}]),
    )
    .await;
    let made = client
        .call(
            "s2",
            "preset.set",
            json!({"changes": [{"key": "preset.name", "value": "我的"}]}),
        )
        .await;
    let mine_id = made["result"]["preset"]
        .as_str()
        .expect("起了编号")
        .to_string();

    let back = client
        .call("d1", "preset.delete", json!({"preset": "dev"}))
        .await;
    assert_eq!(back["result"], json!({"remains": true}), "{back}");
    assert_eq!(mine(&home, "dev"), None);
    let got = client
        .call("g1", "preset.get", json!({"preset": "dev"}))
        .await;
    assert_eq!(on(&got, "memory"), None, "回到出厂的：出厂的没写记忆");
    assert_eq!(got["result"]["remove"], Value::Null);

    let gone = client
        .call("d2", "preset.delete", json!({"preset": mine_id}))
        .await;
    assert_eq!(gone["result"], json!({"remains": false}), "{gone}");
    let got = client
        .call("g2", "preset.get", json!({"preset": mine_id}))
        .await;
    assert_eq!(reason(&got), Some("unknown_preset"), "{got}");

    for (n, id) in ["dev", "full", "nobody"].into_iter().enumerate() {
        let reply = client
            .call(&format!("n{n}"), "preset.delete", json!({"preset": id}))
            .await;
        assert_eq!(reason(&reply), Some("nothing_to_delete"), "{id}：{reply}");
    }
    let reply = client
        .call("x", "preset.delete", json!({"preset": "Bad"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

#[tokio::test]
async fn the_same_value_writes_nothing() {
    let home = Home::new();
    let mut client = connected(&home).await;
    set(
        &mut client,
        "s1",
        "dev",
        json!([{"key": "software.memory", "value": true}]),
    )
    .await;
    let path = home.root.path().join("home/alice/presets/dev.toml");
    let text = "[preset]\nunlisted = 'off'\n\n[software]\nmemory   =   true\n";
    std::fs::write(&path, text).expect("写得进");
    let again = set(
        &mut client,
        "s2",
        "dev",
        json!([
            {"key": "software.memory", "value": true},
            {"key": "preset.unlisted", "value": "off"},
            {"key": "tools.shell", "unset": true},
        ]),
    )
    .await;
    assert_eq!(on(&again, "memory"), Some(true), "{again}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("在"),
        text,
        "这一层本来就是这个值的不动，单引号也不换"
    );
    // 写了编号、还没有的上只删不写：什么都不建。
    let ghost = set(
        &mut client,
        "s3",
        "ghost",
        json!([{"key": "software.net", "unset": true}]),
    )
    .await;
    assert_eq!(reason(&ghost), Some("unknown_preset"), "{ghost}");
    assert_eq!(mine(&home, "ghost"), None);
}

/// P-3 上那几个小时里建的预设写着 `base`（施工 P-3 再补）：照常列出、能用，下一次 `preset.set` 写这份文件时顺手去掉；写错了的
/// 一项，列表里的 `problem` 照连接的语言说、带第几行。
#[tokio::test]
async fn an_old_base_is_ignored_and_dropped_on_the_next_save() {
    let home = Home::new();
    let dir = home.root.path().join("home/alice/presets");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("old.toml"),
        "[preset]\nbase = \"full\"\nname = \"我的\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("broken.toml"), "[preset]\nvoice = 1\n").unwrap();
    let mut client = connected(&home).await;
    let listed = client.call("l", "preset.list", json!({})).await;
    let presets = listed["result"]["presets"].as_array().unwrap();
    let find = |id: &str| {
        presets
            .iter()
            .find(|one| one["preset"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(find("old")["name"], "我的", "{listed}");
    assert!(find("old").get("problem").is_none(), "{listed}");
    let broken = find("broken");
    assert_eq!(broken["problem"], "不认识的键 preset.voice", "{listed}");
    assert_eq!(broken["line"], 2, "{listed}");
    let reply = set(
        &mut client,
        "s",
        "old",
        json!([{"key": "preset.summary", "value": "改过"}]),
    )
    .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        mine(&home, "old").as_deref(),
        Some("[preset]\nname = \"我的\"\nsummary = \"改过\"\n")
    );
}

/// 说明能写空的字（施工 P-3 再补，2026-10-08 项目主人：「为什么说明不让为空？」）：就是没有说明，盖住出厂的那句，`*.get`、
/// 说明能写空的字（施工 P-3 再补，2026-10-08 项目主人：「为什么说明不让为空？」）：就是没有说明，盖住下面那一层的那句，`*.get`、
/// `*.list` 给 `null`；`unset` 才回到下面那一层的。名字照旧不收空的。出厂的不写说明（施工 P-4 下），这里拿系统区的一份试。
#[tokio::test]
async fn an_empty_summary_means_none_and_covers_the_lower_one() {
    let home = Home::new();
    home.write(
        "system/presets/team.toml",
        "[preset]\nname = \"团队\"\nsummary = \"大家的\"\n",
    );
    let mut client = connected(&home).await;
    let reply = set(
        &mut client,
        "s",
        "team",
        json!([{"key": "preset.summary", "value": ""}]),
    )
    .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(reply["result"]["summary"], Value::Null, "{reply}");
    assert_eq!(
        mine(&home, "team").as_deref(),
        Some("[preset]\nsummary = \"\"\n")
    );
    let listed = client.call("l", "preset.list", json!({})).await;
    let team = listed["result"]["presets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|one| one["preset"] == "team")
        .unwrap()
        .clone();
    assert_eq!(team["summary"], Value::Null, "{listed}");
    assert_eq!(team["name"], "团队", "{listed}");
    let back = set(
        &mut client,
        "u",
        "team",
        json!([{"key": "preset.summary", "unset": true}]),
    )
    .await;
    assert_eq!(back["result"]["summary"], "大家的", "{back}");
    let named = set(
        &mut client,
        "n",
        "team",
        json!([{"key": "preset.name", "value": " "}]),
    )
    .await;
    assert_eq!(reason(&named), Some("preset_invalid"), "{named}");
}

/// 预设不再有默认人格（施工 P-4 上，2026-10-08 项目主人：只去掉预设的「默认人格」）：`preset.set` 写这个键参数不对，什么都不写；
/// 旧文件里写了的，下一次写这份文件时去掉；`preset.get` 没有这一格。
#[tokio::test]
async fn the_default_persona_is_gone_from_presets() {
    let home = Home::new();
    let mut client = connected(&home).await;
    for (n, changes) in [
        json!([{"key": "preset.default_persona", "value": "engineer"}]),
        json!([{"key": "preset.default_persona", "unset": true}]),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = set(&mut client, &format!("b{n}"), "dev", changes.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{changes}：{reply}");
    }
    assert_eq!(mine(&home, "dev"), None, "什么都没写");
    let dir = home.root.path().join("home/alice/presets");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("old.toml"),
        "[preset]\nname = \"我的\"\ndefault_persona = \"engineer\"\n",
    )
    .unwrap();
    let got = client
        .call("g", "preset.get", json!({"preset": "old"}))
        .await;
    assert!(got["result"].get("default_persona").is_none(), "{got}");
    let reply = set(
        &mut client,
        "s",
        "old",
        json!([{"key": "preset.summary", "value": "改过"}]),
    )
    .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        mine(&home, "old").as_deref(),
        Some("[preset]\nname = \"我的\"\nsummary = \"改过\"\n")
    );
}
