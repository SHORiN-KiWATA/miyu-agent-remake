//! 密钥的运行日志（施工 8-5，`docs/blueprint/config.md`「出错」）：写、换、删、手改被看到的各记一条
//! `INFO secret changed`，只有名字；Unix 上组、别人读得到的记 `WARN secrets readable by others`；把能走的路都走一遍
//! （写、换、删、列、查、手改、改坏了再写），整份运行日志里搜不到 key，`TRACE` 也搜不到。
//!
//! 全局装一个写进内存的订阅者：监视在它自己的线程上。全局的一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use serde_json::json;

use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::Script;

use support::*;

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";
const FAKE_2: &str = "sk-FAKE-KEY-FOR-TESTS-0002";

const SECRETS: &str = "system/secrets.toml";
const JOURNAL: &str = "system/journal.jsonl";

#[tokio::test]
async fn the_runtime_log_has_names_and_never_a_key() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::TRACE,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let has = |what: &str| memory.lines().iter().any(|line| line.contains(what));

    let home = Home::new();
    home.root.prepare_home(&alice()).expect("建得了家目录");
    home.write(SECRETS, &format!("start = \"{FAKE}\"\n"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = home.root.path().join(SECRETS);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    let core = home.core_configured(&Script::new([]), None, &[]);
    if cfg!(unix) {
        assert!(
            has("WARN  config   secrets readable by others file=system/secrets.toml"),
            "{:?}",
            memory.lines()
        );
    }
    let _watching = core.watch_config().expect("监视得了");
    let mut client = Client::connect(core);
    client.hello().await;
    let calls = [
        ("secret.set", json!({"name": "deepseek", "value": FAKE})),
        ("secret.set", json!({"name": "deepseek", "value": FAKE_2})),
        ("secret.set", json!({"name": "Bad", "value": FAKE})),
        (
            "secret.set",
            json!({"name": "deepseek", "value": format!("{FAKE}\u{7}")}),
        ),
        ("secret.list", json!({})),
        ("config.get", json!({})),
        ("secret.delete", json!({"name": "deepseek"})),
    ];
    for (n, (method, params)) in calls.into_iter().enumerate() {
        let reply = client.call(&format!("secret-{n}"), method, params).await;
        assert!(!reply.to_string().contains("FAKE"), "{reply}");
    }
    home.write(SECRETS, &format!("hand = \"{FAKE}\"\n"));
    until("手改被看到", || {
        has("secret changed name=hand action=set via=file")
    })
    .await;
    home.write(SECRETS, &format!("hand = \"{FAKE}\n"));
    let reply = client
        .call(
            "broken",
            "secret.set",
            json!({"name": "x", "value": FAKE_2}),
        )
        .await;
    assert_eq!(reason(&reply), Some("config_file_broken"));
    assert!(!reply.to_string().contains("FAKE"), "{reply}");

    for line in [
        "INFO  config   secret changed name=deepseek action=set via=set",
        "INFO  config   secret changed name=deepseek action=replaced via=set",
        "INFO  config   secret changed name=deepseek action=deleted via=set",
        "INFO  config   secret changed name=start action=deleted via=file",
    ] {
        assert!(has(line), "{line}：{:?}", memory.lines());
    }
    let log = memory.lines().join("\n");
    assert!(
        log.contains("method=secret.set"),
        "DEBUG request 照记方法名：{log}"
    );
    for key in [FAKE, FAKE_2, "FAKE"] {
        assert!(!log.contains(key), "运行日志里有 key：{log}");
    }
    let journal = std::fs::read_to_string(home.root.path().join(JOURNAL)).unwrap();
    assert!(!journal.contains("FAKE"), "{journal}");
}
