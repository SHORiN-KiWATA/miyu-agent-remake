//! 表单保存契约：顺序、密钥引用和完整价格。
use super::*;
#[test]
fn unchanged_multiple_keys_are_never_replaced_and_plain_secrets_are_not_config_changes() {
    let p = Provider {
        id: "dev".into(),
        name: "dev".into(),
        models: vec![],
    };
    let cfg = json!({"items":{"providers.dev.keys":{"value":[{"env":"A"},{"env":"B"}]}}});
    let mut f = provider(&cfg, Some(&p));
    assert!(f.changes().is_empty());
    f.fields[3].value = "secret-not-for-config".into();
    assert!(f.changes().is_empty());
    assert_eq!(f.fields[3].shown(), "••••••••");
}
#[test]
fn a_new_pool_is_one_transaction_with_ordered_members() {
    let mut f = pool(&json!({}), None);
    f.fields[0].value = "daily".into();
    f.fields[1].value = "pin".into();
    if let Entity::Pool { members, .. } = &mut f.entity {
        *members = vec!["p/b".into(), "p/a".into()];
    }
    let changes = f.changes();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0]["key"], "pools.daily.strategy");
    assert_eq!(changes[1]["value"], json!(["p/b", "p/a"]));
}
fn priced_model() -> (Provider, Model) {
    let p = Provider {
        id: "relay".into(),
        name: "Relay".into(),
        models: vec![],
    };
    let m = Model {
        id: "a.b".into(),
        reference: "relay/a.b".into(),
        facts: json!({
            "effort":{"key":"providers.relay.models.\"a.b\".effort","value":null,"from":"default"},
            "price":{"value":{"input":2,"output":8,"cache_read":1,"cache_write":2,"currency":"CNY"},"from":"catalog"},
            "multiplier":{"value":1.5,"from":"config","layer":"system"}
        }),
    };
    (p, m)
}

#[test]
fn editing_one_catalog_price_saves_the_complete_displayed_base_price() {
    let (p, m) = priced_model();
    let key = "providers.relay.models.\"a.b\".price.input";
    let cfg = json!({"items":{key:{"value":2,"origin":{"layer":"system"},"layers":[{"value":1,"origin":{"layer":"personal"}}]}}});
    let mut f = model(&cfg, &p, &m);
    f.fields
        .iter_mut()
        .find(|f| f.label == "input")
        .unwrap()
        .value = "3".into();
    let changes = f.changes();
    assert_eq!(changes.len(), 5);
    for (name, value) in [
        ("input", "3"),
        ("output", "8"),
        ("cache_read", "1"),
        ("cache_write", "2"),
        ("currency", "CNY"),
    ] {
        let key = format!("providers.relay.models.\"a.b\".price.{name}");
        let change = changes.iter().find(|c| c["key"] == key).unwrap();
        assert_eq!(change["input"], value);
        assert_eq!(
            change["expect"],
            if name == "input" {
                json!({"value":1})
            } else {
                json!({})
            }
        );
    }
}

#[test]
fn multiplier_uses_its_wire_fact_and_catalog_price_has_one_source() {
    let (p, m) = priced_model();
    let f = model(&json!({}), &p, &m);
    let multiplier = f.fields.iter().find(|f| f.label == "multiplier").unwrap();
    assert_eq!(multiplier.value, "1.5");
    assert_eq!(multiplier.source, "config");
    assert_eq!(
        multiplier.key,
        "providers.relay.models.\"a.b\".price_multiplier"
    );
    assert!(
        f.fields
            .iter()
            .filter(|f| f.key.contains(".price."))
            .all(|f| f.source == "catalog")
    );
    assert!(f.changes().is_empty());
}

#[test]
fn clearing_a_rate_unsets_it_and_never_invents_unknown_rates() {
    let (p, mut m) = priced_model();
    m.facts["price"]["value"]["cache_read"] = Value::Null;
    m.facts["price"]["value"]
        .as_object_mut()
        .unwrap()
        .remove("currency");
    let mut f = model(&json!({}), &p, &m);
    assert_eq!(
        f.fields
            .iter()
            .find(|f| f.label == "currency")
            .unwrap()
            .value,
        "USD"
    );
    f.fields
        .iter_mut()
        .find(|f| f.label == "input")
        .unwrap()
        .value
        .clear();
    let changes = f.changes();
    assert_eq!(changes.len(), 5);
    for name in ["input", "cache_read"] {
        let key = format!("providers.relay.models.\"a.b\".price.{name}");
        let c = changes.iter().find(|c| c["key"] == key).unwrap();
        assert_eq!(c["unset"], true);
        assert!(c.get("input").is_none() && c.get("value").is_none());
    }
    assert_eq!(
        changes
            .iter()
            .find(|c| c["key"].as_str().unwrap().ends_with(".currency"))
            .unwrap()["input"],
        "USD"
    );
}

#[test]
fn only_explicit_secret_or_env_references_are_visible_in_key_fields() {
    let mut f = provider(&json!({}), None);
    let key = f
        .fields
        .iter_mut()
        .find(|f| matches!(f.kind, Kind::Key))
        .unwrap();
    key.value = "$ordinary-api-key".into();
    assert!(!key.shown().contains("ordinary-api-key"));
    key.value = "$env:TEST_KEY".into();
    assert_eq!(key.shown(), "$env:TEST_KEY");
}
