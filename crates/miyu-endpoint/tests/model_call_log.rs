//! `model.call` 的运行日志（施工 8-20，`docs/blueprint/models.md`「出错」、「怎么走」第十二条第 6 条）：成了记一行
//! `INFO model call`，带用途、真发给的供应商和模型、输入（三项加起来）、输出；没成的记一行 `INFO model call failed`，带
//! 原因码（`model_failed` 另带分类）；不带会话编号，key 的值不进日志。
//!
//! 全局装一个写进内存的订阅者，一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use serde_json::json;

use miyu_http::testkit::{Reply, Server};
use miyu_log::{LevelFilter, Memory};
use support::providers::{data, profiles, routed, said};
use support::*;

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";

#[tokio::test]
async fn a_call_is_one_line_either_way_and_the_key_is_nowhere() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::TRACE,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let server = Server::start(vec![
        said("Hi."),
        Reply::error(401, &[], r#"{"error":{"message":"bad key"}}"#),
    ])
    .await;
    let home = Home::new();
    home.write(
        "system/config.toml",
        &format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ env = \"A_KEY\" }}\n\n[models]\nchat = \"a/m\"\n",
            server.base_url
        ),
    );
    let core = routed(&home, &[("A_KEY", FAKE)], data(profiles(json!({}))));
    let mut client = Client::connect(core);
    client.hello().await;
    let asked =
        |purpose: &str| json!({"purpose": purpose, "messages": [{"role": "user", "text": "hi"}]});
    let worked = client.call("c1", "model.call", asked("platform")).await;
    assert_eq!(worked["result"]["text"], "Hi.", "{worked}");
    let failed = client.call("c2", "model.call", asked("vision")).await;
    assert_eq!(reason(&failed), Some("model_failed"), "{failed}");
    let mut unknown = asked("vision");
    unknown["model"] = json!("nope/x");
    let unknown = client.call("c3", "model.call", unknown).await;
    assert_eq!(reason(&unknown), Some("unknown_model"), "{unknown}");
    let lines = memory.lines();
    for wanted in [
        " INFO  session  model call purpose=platform provider=a model=m input=12 output=3",
        " INFO  session  model call failed purpose=vision reason=model_failed class=auth",
        " INFO  session  model call failed purpose=vision reason=unknown_model",
    ] {
        assert!(
            lines.iter().any(|line| line.ends_with(wanted)),
            "「{wanted}」应该在 {lines:#?} 里"
        );
    }
    let log = lines.join("\n");
    assert!(!log.contains("FAKE"), "运行日志里有 key：{log}");
}
