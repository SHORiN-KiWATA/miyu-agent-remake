//! 新建、改、删人格，读提示词的原文（施工 P-3 下、P-3 补，`docs/blueprint/personas.md`「改」）：真核心走一遍。只写家目录那
//! 一层；不写编号的是新建、编号由核心起，一次就建好、带着提示词和一对一对的示范对话；`persona.read` 给叠好的原文和你那一层的
//! 版本，`persona.set` 照版本防覆盖；空的字就是这一段是空的；有错的什么都不写、照连接的语言说一句；删了的挪进回收处，下面
//! 还有的回到它们的样子。

use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::Script;

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
async fn a_new_persona_is_made_in_one_call_with_its_prompts() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let made = set(
        &mut client,
        "s1",
        json!({
            "changes": [{"key": "persona.name", "value": "我的"}],
            "prompts": {
                "persona": {"text": "Be brief.\n"},
                "examples": {"pairs": [{"user": "a", "assistant": "b"}, {"user": "c", "assistant": "d\n\ne"}]},
                "reminders": {"text": "Stay.\n"},
            },
        }),
    )
    .await;
    assert_eq!(
        made["result"],
        json!({
            "persona": "persona-1",
            "name": "我的",
            "summary": null,
            "prompts": {"persona": true, "reminders": true},
            "examples": 2,
            "avatar": null,
            "background": null,
            "seed": null,
            "remove": "delete",
        }),
        "{made}"
    );
    assert_eq!(
        mine(&home, "persona-1", "persona.toml").as_deref(),
        Some("[persona]\nname = \"我的\"\n")
    );
    assert_eq!(
        mine(&home, "persona-1", "prompts/examples.md").as_deref(),
        Some("user: a\nassistant: b\n\nuser: c\nassistant: d\ne\n"),
        "核心写成文件的写法，一句里的空行去掉"
    );
    let examples = read(&mut client, "r1", "persona-1", "examples").await;
    assert_eq!(
        examples["pairs"],
        json!([{"user": "a", "assistant": "b"}, {"user": "c", "assistant": "d\ne"}])
    );
    assert!(
        examples["version"]
            .as_str()
            .is_some_and(|version| version.starts_with("sha256:"))
    );
    let reminders = read(&mut client, "r2", "persona-1", "reminders").await;
    assert_eq!(
        reminders,
        json!({"text": "Stay.\n", "version": reminders["version"]})
    );
    assert!(reminders.get("pairs").is_none(), "只有示范对话带 pairs");

    // 一样的字不再写：文件还是原来那一个。
    #[cfg(unix)]
    let before = {
        use std::os::unix::fs::MetadataExt;
        let path = home
            .root
            .path()
            .join("home/alice/personas/persona-1/prompts/reminders.md");
        std::fs::metadata(path).expect("在").ino()
    };
    let same = set(
        &mut client,
        "s2",
        json!({"persona": "persona-1", "prompts": {"reminders": {"text": "Stay.\n"}}}),
    )
    .await;
    assert!(same.get("error").is_none(), "{same}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let path = home
            .root
            .path()
            .join("home/alice/personas/persona-1/prompts/reminders.md");
        assert_eq!(std::fs::metadata(path).expect("在").ino(), before, "没重写");
    }

    let second = set(
        &mut client,
        "s3",
        json!({"changes": [{"key": "persona.name", "value": "又一个"}]}),
    )
    .await;
    assert_eq!(second["result"]["persona"], "persona-2", "{second}");
    let nothing = set(
        &mut client,
        "s4",
        json!({"prompts": {"persona": {"unset": true}}}),
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
            json!({"cwd": home.work.to_string_lossy(), "persona": "persona-1"}),
        )
        .await;
    assert!(created["result"]["session"].is_string(), "{created}");
}

#[tokio::test]
async fn read_gives_the_text_and_your_version_which_guards_saving() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let shipped = read(&mut client, "r1", "engineer", "persona").await;
    assert_eq!(
        shipped,
        json!({"text": "You are a helpful software engineer.\n", "version": null}),
        "来自哪一层不给（施工 P-3 补）"
    );
    let nothing = read(&mut client, "r2", "engineer", "reminders").await;
    assert_eq!(nothing, json!({"text": null, "version": null}));
    let untouched = client
        .call("g0", "persona.get", json!({"persona": "engineer"}))
        .await;
    assert_eq!(
        untouched["result"]["remove"],
        Value::Null,
        "没改过的出厂没什么可删"
    );

    // 改出厂的：你那一层还没有，版本写 null。
    let saved = set(
        &mut client,
        "s1",
        json!({"persona": "engineer", "prompts": {"persona": {"text": "Be terse.\n", "expect": null}}}),
    )
    .await;
    assert_eq!(saved["result"]["remove"], "restore", "{saved}");
    let now = read(&mut client, "r3", "engineer", "persona").await;
    assert_eq!(now["text"], "Be terse.\n");
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
    // 空的字：这一段是空的，不回到出厂的。
    let emptied = set(
        &mut client,
        "s3",
        json!({"persona": "engineer", "prompts": {"persona": {"text": "", "expect": version}}}),
    )
    .await;
    assert_eq!(emptied["result"]["prompts"]["persona"], false, "{emptied}");
    assert_eq!(
        mine(&home, "engineer", "prompts/persona.md").as_deref(),
        Some("")
    );
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
        back["result"]["prompts"]["persona"], true,
        "回到出厂的那一句，没有的删了也不出错：{back}"
    );
    assert_eq!(mine(&home, "engineer", "prompts/persona.md"), None);
    let shipped = read(&mut client, "r1", "engineer", "persona").await;
    assert_eq!(shipped["text"], "You are a helpful software engineer.\n");

    // 空的 pairs 等于删掉。
    set(
        &mut client,
        "s3",
        json!({"persona": "engineer", "prompts": {"examples": {"pairs": [{"user": "a", "assistant": "b"}]}}}),
    )
    .await;
    let cleared = set(
        &mut client,
        "s4",
        json!({"persona": "engineer", "prompts": {"examples": {"pairs": []}}}),
    )
    .await;
    assert_eq!(cleared["result"]["examples"], 0, "{cleared}");
    assert_eq!(mine(&home, "engineer", "prompts/examples.md"), None);
}

#[tokio::test]
async fn mistakes_write_nothing_and_say_why_in_the_peer_language() {
    let home = Home::new();
    let mut client = connected(&home).await;
    for (n, (params, problem, line)) in [
        (
            json!({"persona": "mine", "prompts": {"examples": {"text": "assistant: hi\n"}}}),
            "home prompts/examples.md:1: ",
            Some(1),
        ),
        (
            json!({"persona": "mine", "changes": [{"key": "memory.scope", "value": "off"}]}),
            "home persona.toml:",
            Some(2),
        ),
        (
            json!({"persona": "mine", "prompts": {"examples": {"pairs": [{"user": "a", "assistant": "照这样：\nuser: 你好"}]}}}),
            "prompts/examples.md: pair 1: ",
            None,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = set(&mut client, &format!("i{n}"), params.clone()).await;
        assert_eq!(reason(&reply), Some("persona_invalid"), "{params}：{reply}");
        let data = &reply["error"]["data"];
        assert!(data["problem"].as_str().unwrap_or_default().starts_with(problem), "{reply}");
        let told = data["message"].as_str().unwrap_or_default();
        assert!(!told.is_empty() && !told.contains("home"), "照连接的语言、不带层：{reply}");
        assert_eq!(data["line"].as_u64(), line.map(|line: u64| line), "{reply}");
    }
    assert!(
        !home.root.path().join("home/alice/personas/mine").exists(),
        "什么都没写"
    );
    for (n, params) in [
        json!({"persona": "mine"}),
        json!({"changes": [], "prompts": {}}),
        json!({"persona": "mine", "prompts": {"voice": {"text": "x"}}}),
        json!({"persona": "mine", "prompts": {"persona": {"text": "x", "unset": true}}}),
        json!({"persona": "mine", "prompts": {"persona": {"unset": false}}}),
        json!({"persona": "mine", "prompts": {"persona": {"text": "x", "expect": 3}}}),
        json!({"persona": "mine", "prompts": {"persona": {"pairs": [{"user": "a", "assistant": "b"}]}}}),
        json!({"persona": "mine", "prompts": {"examples": {"pairs": [{"user": " ", "assistant": "b"}]}}}),
        json!({"persona": "mine", "prompts": {"examples": {"pairs": [{"user": "a", "assistant": "b", "x": 1}]}}}),
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
    let made = set(
        &mut client,
        "s1",
        json!({"prompts": {"persona": {"text": "Mine.\n"}}}),
    )
    .await;
    let id = made["result"]["persona"]
        .as_str()
        .expect("起了编号")
        .to_string();
    set(
        &mut client,
        "s2",
        json!({"persona": "engineer", "prompts": {"persona": {"text": "Mine too.\n"}}}),
    )
    .await;

    let gone = client
        .call("d1", "persona.delete", json!({"persona": id}))
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
    assert!(trashed[0].starts_with(&format!("{id}.")), "{trashed:?}");
    assert_eq!(
        std::fs::read_to_string(trash.join(&trashed[0]).join("prompts/persona.md")).expect("在"),
        "Mine.\n",
        "整个目录挪过去"
    );
    let got = client
        .call("g1", "persona.get", json!({"persona": id}))
        .await;
    assert_eq!(reason(&got), Some("unknown_persona"), "{got}");

    let back = client
        .call("d2", "persona.delete", json!({"persona": "engineer"}))
        .await;
    assert_eq!(back["result"], json!({"remains": true}), "{back}");
    let shipped = read(&mut client, "r1", "engineer", "persona").await;
    assert_eq!(shipped["text"], "You are a helpful software engineer.\n");

    for (n, persona) in ["engineer", "nobody"].into_iter().enumerate() {
        let reply = client
            .call(
                &format!("n{n}"),
                "persona.delete",
                json!({"persona": persona}),
            )
            .await;
        assert_eq!(
            reason(&reply),
            Some("nothing_to_delete"),
            "{persona}：{reply}"
        );
    }
    let reply = client
        .call("x", "persona.delete", json!({"persona": "Bad"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

/// P-3 上那几个小时里建的人格写着 `base`（施工 P-3 再补）：照常列出、能用，下一次 `persona.set` 写这份文件时顺手去掉；写错了的
/// 一项，列表里的 `problem` 照连接的语言说、带第几行。
#[tokio::test]
async fn an_old_base_is_ignored_and_dropped_on_the_next_save() {
    let home = Home::new();
    let dir = home.root.path().join("home/alice/personas");
    std::fs::create_dir_all(dir.join("old")).unwrap();
    std::fs::write(
        dir.join("old/persona.toml"),
        "[persona]\nbase = \"none\"\nname = \"阿米\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("broken")).unwrap();
    std::fs::write(dir.join("broken/persona.toml"), "[persona]\n\nvoice = 1\n").unwrap();
    let mut client = connected(&home).await;
    let listed = client.call("l", "persona.list", json!({})).await;
    let personas = listed["result"]["personas"].as_array().unwrap();
    let find = |id: &str| {
        personas
            .iter()
            .find(|one| one["persona"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(find("old")["name"], "阿米", "{listed}");
    assert!(find("old").get("problem").is_none(), "{listed}");
    let broken = find("broken");
    assert_eq!(broken["problem"], "不认识的键 persona.voice", "{listed}");
    assert_eq!(broken["line"], 3, "{listed}");
    let reply = set(
        &mut client,
        "s",
        json!({"persona": "old", "changes": [{"key": "persona.summary", "value": "我的"}]}),
    )
    .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        mine(&home, "old", "persona.toml").as_deref(),
        Some("[persona]\nname = \"阿米\"\nsummary = \"我的\"\n")
    );
}

/// 说明能写空的字（施工 P-3 再补）：就是没有说明，盖住下面那一层的，`persona.get` 给 `null`；名字照旧不收空的。
#[tokio::test]
async fn an_empty_summary_means_none() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let reply = set(
        &mut client,
        "s",
        json!({"persona": "engineer", "changes": [{"key": "persona.summary", "value": ""}]}),
    )
    .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(reply["result"]["summary"], Value::Null, "{reply}");
    assert!(reply["result"]["name"].is_string(), "{reply}");
    let named = set(
        &mut client,
        "n",
        json!({"persona": "engineer", "changes": [{"key": "persona.name", "value": ""}]}),
    )
    .await;
    assert_eq!(reason(&named), Some("persona_invalid"), "{named}");
}
