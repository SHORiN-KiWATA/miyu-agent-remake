//! 扩展能力的审批（施工 9-4 下上，`docs/blueprint/extensions.md`「能力」，`05-内核接口.md` 第三节）：清单的
//! `[process] capabilities` 声明要哪些，管理员在 `extension.enable` 那一下带上 `approve` 批；批过的记在开关的文件里
//! （批的时候声明的全部）。还没批的是「声明的减批过的」：升级以后多了的只问多的。出厂的包（资源目录里随 Miyu 装的）和
//! 编译进核心的模块一样本来就被信任，算批过的。

use serde_json::{Value, json};

use miyu_config::package::{Capability, Manifest};
use miyu_config::words::Words;
use miyu_store::extensions::Switches;
use miyu_store::packages::Layer;

use crate::Core;
use crate::refusal::Refusal;

/// 包声明的能力，照能力表的先后。
fn declared(manifest: &Manifest) -> &[Capability] {
    manifest
        .process
        .as_ref()
        .map_or(&[], |process| process.capabilities.as_slice())
}

/// 包 `id` 是不是出厂的。
fn shipped(core: &Core, id: &str) -> bool {
    core.packages
        .iter()
        .any(|found| found.id == id && found.layer == Layer::Shipped)
}

/// 包 `id` 还没批的能力，照能力表的先后：出厂的没有。
pub(super) fn unapproved(
    core: &Core,
    id: &str,
    manifest: &Manifest,
    switches: &Switches,
) -> Vec<Capability> {
    if shipped(core, id) {
        return Vec::new();
    }
    let approved = switches.approved.get(id);
    declared(manifest)
        .iter()
        .copied()
        .filter(|capability| {
            !approved.is_some_and(|names| names.iter().any(|name| name == capability.as_str()))
        })
        .collect()
}

/// 开包 `id` 之前批：`approve` 里有不是它声明的名字的 `bad_params`；还有没批、`approve` 没盖住的 `needs_approval`
/// （`data.capabilities` 是没盖住的那几个）；盖住了的把它这时声明的全部记进 `switches`。都批过了的不动。
pub(super) fn approve(
    core: &Core,
    id: &str,
    manifest: &Manifest,
    switches: &mut Switches,
    approve: Option<&[String]>,
) -> Result<(), Refusal> {
    let declared = declared(manifest);
    let named = approve.unwrap_or_default();
    let known = |name: &String| Capability::parse(name).is_some_and(|it| declared.contains(&it));
    if !named.iter().all(known) {
        return Err(Refusal::BAD_PARAMS);
    }
    let missing = unapproved(core, id, manifest, switches);
    if missing.is_empty() {
        return Ok(());
    }
    let left: Vec<Capability> = missing
        .into_iter()
        .filter(|capability| !named.iter().any(|name| name == capability.as_str()))
        .collect();
    if !left.is_empty() {
        return Err(needs_approval(&left));
    }
    let all = declared.iter().map(|it| it.as_str().to_string()).collect();
    switches.approved.insert(id.to_string(), all);
    Ok(())
}

/// 还有没批的：拒绝 `needs_approval`，`data.capabilities` 是这几个。
pub(super) fn needs_approval(left: &[Capability]) -> Refusal {
    let names: Vec<&str> = left.iter().map(|it| it.as_str()).collect();
    Refusal::needs_approval(json!(names))
}

/// 给人看的那两格：声明的每一个 `{"id", "name", "summary"}`（名字、一句说明照 `language` 挑，没有字的名字是编号、说明是
/// `null`），还有没批的名字。没声明能力的包都没有。
pub(super) fn shown(
    core: &Core,
    id: &str,
    manifest: &Manifest,
    switches: &Switches,
    language: &str,
) -> Option<(Value, Option<Value>)> {
    let declared = declared(manifest);
    if declared.is_empty() {
        return None;
    }
    let words = crate::config::methods::words(core, language).ok();
    let said = |key: String| {
        words
            .as_ref()
            .and_then(|words| Words::sentence(words, &key, &[]))
    };
    let listed: Vec<Value> = declared
        .iter()
        .map(|capability| {
            let key = format!("extensions/capabilities/{}", capability.as_str());
            json!({
                "id": capability.as_str(),
                "name": said(key.clone()).unwrap_or_else(|| capability.as_str().to_string()),
                "summary": said(format!("{key}/summary")),
            })
        })
        .collect();
    let left = unapproved(core, id, manifest, switches);
    let left =
        (!left.is_empty()).then(|| json!(left.iter().map(|it| it.as_str()).collect::<Vec<_>>()));
    Some((json!(listed), left))
}
