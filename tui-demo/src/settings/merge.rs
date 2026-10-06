//! 界面上看的那一份：读来的数据叠上草稿（蓝图「配置页」第 16 条）。新加的供应商、模型、池列进去，删了的拿掉，改了的显示名、
//! 池、默认用途、模型资料照草稿写。列表、详情、选模型的窗都只读这一份。

use serde_json::{Value, json};

use super::data::{Data, Model, Pool, Provider};
use super::draft::Draft;
use super::keys;

/// 叠好的一份。
pub fn merge(data: &Data, draft: &Draft) -> Data {
    let mut view = data.clone();
    view.providers
        .retain(|p| !draft.removes(&keys::provider(&p.id), data));
    for id in &draft.new_providers {
        if view.provider(id).is_none() {
            view.providers.push(Provider {
                id: id.clone(),
                ..Provider::default()
            });
        }
    }
    for (id, name) in &draft.new_models {
        if let Some(p) = view.providers.iter_mut().find(|p| &p.id == id)
            && !p.models.iter().any(|m| &m.name == name)
        {
            p.models.push(Model {
                name: name.clone(),
                reference: format!("{id}/{name}"),
                listed: vec!["config".into()],
                facts: json!({}),
                table: keys::model_table(id, name),
            });
        }
    }
    for p in &mut view.providers {
        p.models.retain(|m| !draft.removes(&m.table, data));
        let name_key = format!("{}.name", keys::provider(&p.id));
        if draft.changed(&name_key) {
            p.name = draft
                .value(&name_key, data)
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        for m in &mut p.models {
            overlay_facts(m, draft);
        }
    }
    view.pools
        .retain(|p| !draft.removes(&keys::pool(&p.name), data));
    for name in &draft.new_pools {
        if view.pool(name).is_none() {
            view.pools.push(Pool {
                name: name.clone(),
                ..Pool::default()
            });
        }
    }
    for pool in &mut view.pools {
        let table = keys::pool(&pool.name);
        if let Some(members) = draft.value(&format!("{table}.models"), data) {
            pool.members = strings(members);
        }
        if let Some(strategy) = draft.value(&format!("{table}.strategy"), data) {
            pool.strategy = strategy.as_str().map(str::to_string);
        }
    }
    if draft.changed("models.chat") {
        view.chat = text(draft.value("models.chat", data));
    }
    if draft.changed("models.vision") {
        view.vision = text(draft.value("models.vision", data));
    }
    view
}

/// 草稿里写了这个模型的窗口、能收什么：列表里照草稿显示。
fn overlay_facts(model: &mut Model, draft: &Draft) {
    for (key, value) in draft.sets() {
        let Some(item) = key
            .strip_prefix(model.table.as_str())
            .and_then(|r| r.strip_prefix('.'))
        else {
            continue;
        };
        if matches!(item, "window" | "inputs") {
            model.facts[item] = json!({"value": value, "from": "config", "layer": "personal"});
        }
    }
}

fn text(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).map(str::to_string)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::merge;
    use crate::settings::draft::Draft;
    use crate::settings::test_support::sample;

    #[test]
    fn the_view_shows_what_the_draft_adds_removes_and_renames() {
        let data = sample();
        let mut draft = Draft::default();
        draft.new_providers.push("zen".into());
        draft.set(
            "providers.zen.base_url",
            json!("https://zen.invalid/v1"),
            &data,
        );
        draft.new_models.push(("dev".into(), "pro".into()));
        draft.set("providers.dev.models.pro.window", json!(64000), &data);
        draft.set("providers.relay.name", json!("Relay"), &data);
        draft.unset_all("pools.daily", &data);
        draft.set("models.chat", json!("dev/pro"), &data);
        let view = merge(&data, &draft);
        let ids: Vec<&str> = view.providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["dev", "relay", "zen"]);
        assert_eq!(view.provider("relay").unwrap().shown(), "Relay");
        let (_, pro) = view.model("dev/pro").unwrap();
        assert_eq!(pro.window(), Some(64000));
        assert!(pro.custom());
        assert!(view.pool("daily").is_none(), "删了的池不列");
        assert_eq!(view.chat.as_deref(), Some("dev/pro"));
    }

    #[test]
    fn a_provider_whose_personal_keys_are_all_unset_is_not_listed() {
        let data = sample();
        let mut draft = Draft::default();
        draft.unset_all("providers.relay", &data);
        let view = merge(&data, &draft);
        assert!(
            view.provider("relay").is_none(),
            "个人层写的都删了：照删了显示（写在系统层的，删除时先拦下，不会走到这里）"
        );
    }
}
