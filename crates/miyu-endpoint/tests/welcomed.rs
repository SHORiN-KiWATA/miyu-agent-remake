//! 第一次引导走过了（施工 8-11 四补，`docs/blueprint/config.md`「M8 的配置项」）：`ui.welcomed` 账号级、只在个人设置里，
//! 设置页不画（Schema 里带 `hidden`），头走完引导写 `true`、经 `config.get` 读；写进系统配置的不收。

use serde_json::json;

use miyu_session::testkit::Script;

use crate::support::*;

#[tokio::test]
async fn the_welcome_flag_is_personal_hidden_and_written_by_the_heads() {
    let home = Home::new();
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    let schema = client
        .call("s1", "config.schema", json!({"keys": ["ui.welcomed"]}))
        .await;
    let item = &schema["result"]["items"][0];
    assert_eq!(
        (
            &item["key"],
            &item["type"],
            &item["default"],
            &item["layers"],
            &item["hidden"],
            &item["name"]
        ),
        (
            &json!("ui.welcomed"),
            &json!("bool"),
            &json!(false),
            &json!(["personal"]),
            &json!(true),
            &json!("第一次引导走过了")
        ),
        "{schema}"
    );
    let before = client.call("g1", "config.get", json!({})).await;
    assert!(
        before["result"]["items"]
            .get("ui.welcomed")
            .is_none_or(|item| item["value"] == false),
        "不写是假：{before}"
    );

    let written = client
        .call(
            "w1",
            "config.set",
            json!({"layer": "personal", "changes": [{"key": "ui.welcomed", "value": true}]}),
        )
        .await;
    assert!(written.get("error").is_none(), "{written}");
    let after = client.call("g2", "config.get", json!({})).await;
    assert_eq!(
        after["result"]["items"]["ui.welcomed"]["value"], true,
        "{after}"
    );
    let personal = std::fs::read_to_string(home.root.path().join("home/alice/settings.toml"))
        .expect("写进了个人设置");
    assert!(personal.contains("[ui]\nwelcomed = true\n"), "{personal}");

    let system = client
        .call(
            "w2",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "ui.welcomed", "value": true}]}),
        )
        .await;
    assert_eq!(reason(&system), Some("config_invalid"), "{system}");
    assert_eq!(
        system["error"]["data"]["problems"][0]["code"], "wrong_layer",
        "账号级：系统配置里不收"
    );
}
