use serde_json::json;

use super::Draft;
use crate::settings::test_support::sample;

#[test]
fn setting_back_what_the_personal_layer_has_is_no_change() {
    let data = sample();
    let mut draft = Draft::default();
    let key = "providers.dev.models.flash.window";
    draft.set(key, json!(64000), &data);
    assert!(draft.changed(key));
    assert_eq!(draft.value(key, &data), Some(&json!(64000)));
    draft.set(key, json!(128000), &data);
    assert!(draft.is_empty(), "改回原样不算改动");
    draft.unset("providers.dev.name", &data);
    assert!(draft.is_empty(), "个人层没写的删了也不算");
}

#[test]
fn providers_whose_connection_changed_get_their_model_list_again() {
    let data = sample();
    let mut draft = Draft::default();
    draft.new_providers.push("zen".into());
    draft.set(
        "providers.relay.base_url",
        json!("https://other.invalid/v1"),
        &data,
    );
    draft.set("providers.relay.name", json!("Relay"), &data);
    draft.set_secret("dev", "sk-dev".into());
    draft.set("providers.dev.models.\"v4.1\".window", json!(1), &data);
    assert_eq!(
        draft.reconnects(&data),
        ["dev", "relay", "zen"],
        "引号里的点不算分段"
    );
    // 删掉的那家不再去取（2026-10-07 项目主人报：删了 test 底下出一句「没取到模型列表」）。
    let mut removing = Draft::default();
    removing.unset_all("providers.relay", &data);
    assert!(removing.reconnects(&data).is_empty());
}

#[test]
fn deleting_a_table_unsets_every_personal_key_under_it() {
    let data = sample();
    let mut draft = Draft::default();
    draft.set("pools.daily.models", json!(["dev/flash"]), &data);
    draft.unset_all("pools.daily", &data);
    assert!(draft.removes("pools.daily", &data));
    let sent = draft.request(&data, &[]);
    assert_eq!(
        sent["changes"],
        json!([
            {"key": "pools.daily.models", "unset": true, "expect": {"value": ["dev/flash", "relay/cline/deepseek-v4"]}},
            {"key": "pools.daily.strategy", "unset": true, "expect": {"value": "rotate"}}
        ])
    );
}

#[test]
fn a_new_key_gets_a_fresh_secret_name_and_the_reference_goes_into_key() {
    let data = sample();
    let mut draft = Draft::default();
    draft.set_secret("relay", "sk-relay".into());
    draft.set_secret("dev", "sk-dev".into());
    let secrets = draft.secrets(&data);
    assert_eq!(
        secrets,
        vec![
            ("dev".into(), "dev-key".into(), "sk-dev".into()),
            ("relay".into(), "relay-key-2".into(), "sk-relay".into())
        ],
        "已有的 relay-key 不覆盖"
    );
    let named = [("relay".to_string(), "relay-key-2".to_string())];
    let sent = draft.request(&data, &named);
    assert_eq!(sent["layer"], "personal");
    assert_eq!(
        sent["changes"],
        json!([{"key": "providers.relay.key", "value": {"secret": "relay-key-2"},
                "expect": {"value": {"secret": "relay-key"}}}])
    );
    assert!(
        !sent.to_string().contains("sk-relay"),
        "明文不进 config.set"
    );
}
