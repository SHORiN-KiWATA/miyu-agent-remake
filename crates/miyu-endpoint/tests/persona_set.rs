//! 新建、改、删人格，读提示词的原文（施工 P-3 下，`docs/blueprint/personas.md`「改」）：真核心走一遍。只写家目录那一层；
//! 一次就建好、带着提示词；`persona.read` 给叠好的原文、来自哪儿、你那一层的版本，`persona.set` 照版本防覆盖；有错的什么
//! 都不写；删了的挪进回收处，下面还有的回到它们的样子。

mod support;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use support::*;

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// 家目录里人格 `id` 的一份，没有的是空的。
fn mine(home: &Home, id: &str, file: &str) -> Option<String> {
    std::fs::read_to_string(
        home.root
            .path()
            .join(format!("home/alice/personas/{id}/{file}")),
    )
    .ok()
}

async fn set(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "persona.set", params).await
}

async fn read(client: &mut Client, id: &str, persona: &str, prompt: &str) -> Value {
    let reply = client
        .call(
            id,
            "persona.read",
            json!({"persona": persona, "prompt": prompt}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    reply["result"].clone()
}

#[tokio::test]
async fn a_new_persona_is_made_with_its_prompts_in_one_call() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let made = set(
        &mut client,
        "s1",
        json!({
            "persona": "mine",
            "changes": [
                {"key": "persona.base", "value": "engineer"},
                {"key": "persona.name.zh", "value": "我的"},
            ],
            "prompts": {"examples": {"text": "user: a\nassistant: b\n"}},
        }),
    )
    .await;
    let made = &made["result"];
    assert_eq!(made["base"], "engineer", "{made}");
    assert_eq!(made["layers"], json!(["home"]));
    assert_eq!(
        made["prompts"],
        json!({"persona": "base:engineer/shipped", "examples": "home", "reminders": null})
    );
    assert_eq!(made["examples"], 1);
    assert_eq!(
        mine(&home, "mine", "persona.toml").as_deref(),
        Some("[persona]\nbase = \"engineer\"\n\n[persona.name]\nzh = \"我的\"\n")
    );
    assert_eq!(
        mine(&home, "mine", "prompts/examples.md").as_deref(),
        Some("user: a\nassistant: b\n")
    );
    let examples = read(&mut client, "r1", "mine", "examples").await;
    assert_eq!(
        (&examples["text"], &examples["from"]),
        (&json!("user: a\nassistant: b\n"), &json!({"layer": "home"}))
    );
    let reminders = read(&mut client, "r2", "mine", "reminders").await;
    assert_eq!(
        reminders,
        json!({"text": null, "from": null, "version": null}),
        "有示范对话、没有角色扮演提示"
    );

    // 一样的字不再写：文件还是原来那一个。
    #[cfg(unix)]
    let before = {
        use std::os::unix::fs::MetadataExt;
        let path = home
            .root
            .path()
            .join("home/alice/personas/mine/prompts/examples.md");
        std::fs::metadata(path).expect("在").ino()
    };
    let same = set(
        &mut client,
        "s2",
        json!({"persona": "mine", "prompts": {"examples": {"text": "user: a\nassistant: b\n"}}}),
    )
    .await;
    assert!(same.get("error").is_none(), "{same}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let path = home
            .root
            .path()
            .join("home/alice/personas/mine/prompts/examples.md");
        assert_eq!(std::fs::metadata(path).expect("在").ino(), before, "没重写");
    }

    let created = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": home.work.to_string_lossy(), "persona": "mine"}),
        )
        .await;
    assert!(created["result"]["session"].is_string(), "{created}");
}

#[tokio::test]
async fn read_gives_the_text_where_it_comes_from_and_your_version() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let shipped = read(&mut client, "r1", "engineer", "persona").await;
    assert_eq!(
        shipped,
        json!({
            "text": "You are a helpful software engineer.\n",
            "from": {"layer": "shipped"},
            "version": null,
        })
    );
    let nothing = read(&mut client, "r2", "engineer", "reminders").await;
    assert_eq!(
        nothing,
        json!({"text": null, "from": null, "version": null})
    );

    // 改出厂的：你那一层还没有，版本写 null。
    let saved = set(
        &mut client,
        "s1",
        json!({"persona": "engineer", "prompts": {"persona": {"text": "Be terse.\n", "expect": null}}}),
    )
    .await;
    assert_eq!(saved["result"]["prompts"]["persona"], "home", "{saved}");
    let now = read(&mut client, "r3", "engineer", "persona").await;
    assert_eq!(
        (&now["text"], &now["from"]),
        (&json!("Be terse.\n"), &json!({"layer": "home"}))
    );
    let version = now["version"].as_str().expect("有版本").to_string();
    assert!(version.starts_with("sha256:"), "{version}");

    // 拿着旧的版本再存：撞上了，什么都不写。
    let stale = set(
        &mut client,
        "s2",
        json!({"persona": "engineer", "prompts": {"persona": {"text": "Lost.\n", "expect": null}}}),
    )
    .await;
    assert_eq!(reason(&stale), Some("persona_conflict"), "{stale}");
    assert_eq!(stale["error"]["data"]["current"], json!(version));
    assert_eq!(
        mine(&home, "engineer", "prompts/persona.md").as_deref(),
        Some("Be terse.\n")
    );
    let fresh = set(
        &mut client,
        "s3",
        json!({"persona": "engineer", "prompts": {"persona": {"text": "Be kind.\n", "expect": version}}}),
    )
    .await;
    assert!(fresh.get("error").is_none(), "{fresh}");

    // 来自底的，写明底是谁。
    set(
        &mut client,
        "s4",
        json!({"persona": "mine", "changes": [{"key": "persona.base", "value": "engineer"}]}),
    )
    .await;
    let through = read(&mut client, "r4", "mine", "persona").await;
    assert_eq!(
        through["from"],
        json!({"layer": "home", "base": "engineer"})
    );
    assert_eq!(through["text"], "Be kind.\n");
    assert_eq!(through["version"], Value::Null, "mine 自己那一层没有这一份");
}

#[tokio::test]
async fn unsetting_goes_back_to_the_lower_layer() {
    let home = Home::new();
    let mut client = connected(&home).await;
    set(
        &mut client,
        "s1",
        json!({"persona": "engineer", "prompts": {"persona": {"text": "Mine.\n"}}}),
    )
    .await;
    let back = set(
        &mut client,
        "s2",
        json!({
            "persona": "engineer",
            "prompts": {"persona": {"unset": true}, "reminders": {"unset": true}},
        }),
    )
    .await;
    assert_eq!(
        back["result"]["prompts"]["persona"], "shipped",
        "没有的那一份删了也不出错：{back}"
    );
    assert_eq!(mine(&home, "engineer", "prompts/persona.md"), None);
}

#[tokio::test]
async fn mistakes_write_nothing() {
    let home = Home::new();
    let mut client = connected(&home).await;
    for (n, (params, problem)) in [
        (
            json!({"persona": "mine", "prompts": {"examples": {"text": "assistant: hi\n"}}}),
            "home prompts/examples.md:1: ",
        ),
        (
            json!({"persona": "mine", "changes": [{"key": "memory.scope", "value": "off"}]}),
            "home persona.toml:",
        ),
        (
            json!({"persona": "mine", "changes": [{"key": "persona.base", "value": "mine"}]}),
            "base cycle: mine -> mine",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = set(&mut client, &format!("i{n}"), params.clone()).await;
        assert_eq!(reason(&reply), Some("persona_invalid"), "{params}：{reply}");
        let said = reply["error"]["data"]["problem"]
            .as_str()
            .unwrap_or_default();
        assert!(said.starts_with(problem), "{said}");
    }
    assert!(
        !home.root.path().join("home/alice/personas/mine").exists(),
        "什么都没写"
    );
    for (n, params) in [
        json!({"persona": "mine"}),
        json!({"persona": "mine", "changes": [], "prompts": {}}),
        json!({"persona": "mine", "prompts": {"voice": {"text": "x"}}}),
        json!({"persona": "mine", "prompts": {"persona": {"text": "x", "unset": true}}}),
        json!({"persona": "mine", "prompts": {"persona": {"unset": false}}}),
        json!({"persona": "mine", "prompts": {"persona": {"text": "x", "expect": 3}}}),
        json!({"persona": "Mine", "prompts": {"persona": {"text": "x"}}}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = set(&mut client, &format!("b{n}"), params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    for (n, params) in [
        json!({"persona": "engineer", "prompt": "voice"}),
        json!({"persona": "engineer"}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client
            .call(&format!("r{n}"), "persona.read", params.clone())
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    let reply = client
        .call(
            "r9",
            "persona.read",
            json!({"persona": "nobody", "prompt": "persona"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("unknown_persona"), "{reply}");
}

#[tokio::test]
async fn deleting_moves_your_layer_into_the_trash() {
    let home = Home::new();
    let mut client = connected(&home).await;
    set(
        &mut client,
        "s1",
        json!({"persona": "mine", "prompts": {"persona": {"text": "Mine.\n"}}}),
    )
    .await;
    set(
        &mut client,
        "s2",
        json!({"persona": "engineer", "prompts": {"persona": {"text": "Mine too.\n"}}}),
    )
    .await;

    let gone = client
        .call("d1", "persona.delete", json!({"persona": "mine"}))
        .await;
    assert_eq!(gone["result"], json!({"remains": false}), "{gone}");
    let trash = home.root.path().join("home/alice/trash/personas");
    let trashed: Vec<String> = std::fs::read_dir(&trash)
        .expect("回收处建了")
        .map(|entry| {
            entry
                .expect("读得出")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(trashed.len(), 1);
    assert!(trashed[0].starts_with("mine."), "{trashed:?}");
    assert_eq!(
        std::fs::read_to_string(trash.join(&trashed[0]).join("prompts/persona.md")).expect("在"),
        "Mine.\n",
        "整个目录挪过去"
    );
    let got = client
        .call("g1", "persona.get", json!({"persona": "mine"}))
        .await;
    assert_eq!(reason(&got), Some("unknown_persona"), "{got}");

    let back = client
        .call("d2", "persona.delete", json!({"persona": "engineer"}))
        .await;
    assert_eq!(back["result"], json!({"remains": true}), "{back}");
    let shipped = read(&mut client, "r1", "engineer", "persona").await;
    assert_eq!(shipped["from"], json!({"layer": "shipped"}));

    for (n, id) in ["engineer", "nobody"].into_iter().enumerate() {
        let reply = client
            .call(&format!("n{n}"), "persona.delete", json!({"persona": id}))
            .await;
        assert_eq!(reason(&reply), Some("nothing_to_delete"), "{id}：{reply}");
    }
    let reply = client
        .call("x", "persona.delete", json!({"persona": "Bad"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}
