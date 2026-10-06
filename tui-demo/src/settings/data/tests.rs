use serde_json::json;

use crate::settings::test_support::sample;

#[test]
fn providers_models_pools_and_uses_come_from_the_model_list() {
    let data = sample();
    assert_eq!(data.providers.len(), 2);
    let relay = data.provider("relay").unwrap();
    assert_eq!(relay.shown(), "中转站");
    assert_eq!(relay.keys, vec![("secret:relay-key".to_string(), true)]);
    let (_, glm) = data.model("relay/zhipu/glm-5v").unwrap();
    assert!(glm.sees());
    assert_eq!(glm.window(), Some(64000));
    assert_eq!(
        glm.table, "providers.relay.models.\"zhipu/glm-5v\"",
        "模型那张表的键照核心给的 effort 键"
    );
    assert_eq!(data.pool("daily").unwrap().members.len(), 2);
    assert_eq!(data.chat.as_deref(), Some("@daily"));
    assert_eq!(data.vision.as_deref(), Some("relay/zhipu/glm-5v"));
}

#[test]
fn only_models_listed_by_the_config_alone_are_custom() {
    let data = sample();
    assert!(data.model("dev/flash").unwrap().1.custom());
    assert!(!data.model("relay/gpt-oss").unwrap().1.custom());
}

#[test]
fn the_personal_layer_and_system_keys_are_remembered_for_expect_and_deletes() {
    let data = sample();
    assert_eq!(
        data.personal.get("providers.relay.keys"),
        Some(&json!([{"secret": "relay-key"}]))
    );
    assert!(data.in_system("providers.relay"), "地址写在系统层");
    assert!(!data.in_system("providers.dev"));
    assert_eq!(
        data.personal_under("pools.daily"),
        vec!["pools.daily.models", "pools.daily.strategy"]
    );
    assert!(data.secrets.contains("relay-key"));
}

#[test]
fn a_fact_from_the_config_says_which_layer() {
    let data = sample();
    let (_, flash) = data.model("dev/flash").unwrap();
    assert_eq!(flash.from("window"), "config:personal");
    assert_eq!(flash.from("inputs"), "default");
    assert_eq!(flash.from("nothing"), "default");
}
