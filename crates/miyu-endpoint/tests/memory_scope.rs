//! 记忆的范围（施工 R-3 下，`docs/blueprint/memory.md`「范围」，`protocol.md` 的 `session.create`）：真核心走一遍。`memory`
//! 三种照写的记进快照，写错的参数不对、什么都不造；不写的照人格的 `persona.toml`，那也没写的跟着人格。

mod support;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_policy::Snapshot;
use miyu_policy::memory::MemoryScope;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;

use support::*;

/// 会话 `session` 快照里记忆的范围。
fn scope(home: &Home, session: &str) -> MemoryScope {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂").memory_scope()
}

async fn create(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "session.create", params).await
}

#[tokio::test]
async fn session_create_records_the_scope_and_refuses_a_wrong_one() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    for (id, memory, expected) in [
        ("c1", json!("persona"), MemoryScope::Persona),
        ("c2", json!("session"), MemoryScope::Session),
        ("c3", json!("off"), MemoryScope::Off),
        ("c4", Value::Null, MemoryScope::Persona),
    ] {
        let reply = create(&mut client, id, json!({"cwd": "~", "memory": memory})).await;
        let session = reply["result"]["session"].as_str().expect("造了");
        assert_eq!(scope(&home, session), expected, "{memory}");
    }
    for (id, memory) in [
        ("c5", json!("Off")),
        ("c6", json!("everywhere")),
        ("c7", json!(3)),
    ] {
        let reply = create(&mut client, id, json!({"cwd": "~", "memory": memory})).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{memory}：{reply}");
    }
    let listed = client.call("l1", "session.list", json!({})).await;
    assert_eq!(
        listed["result"]["sessions"].as_array().map(Vec::len),
        Some(4),
        "写错的什么都没造"
    );
}

#[tokio::test]
async fn without_it_the_persona_file_decides() {
    let home = Home::new();
    home.write(
        "home/alice/personas/engineer/persona.toml",
        "[memory]\nscope = \"session\"\n",
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = create(&mut client, "c1", json!({"cwd": "~"})).await;
    let session = reply["result"]["session"].as_str().expect("造了");
    assert_eq!(scope(&home, session), MemoryScope::Session, "照人格的");
    let reply = create(&mut client, "c2", json!({"cwd": "~", "memory": "persona"})).await;
    let session = reply["result"]["session"].as_str().expect("造了");
    assert_eq!(
        scope(&home, session),
        MemoryScope::Persona,
        "写了的压着人格的"
    );
}
