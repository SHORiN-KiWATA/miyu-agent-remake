//! `preset.get` 的一个个功能（施工 F-3 下，设计 `30-插件框架.md` 第四节，`docs/blueprint/presets.md`「怎么走」第 8 条）：
//! 装了的功能照清单读的先后，每个带开不开、下面的每件工具开不开；预设里写了、没装的接在后面。头照它一个功能一个开关画，
//! 展开能逐件关。

use std::collections::BTreeSet;

use serde_json::{Value, json};

use miyu_config::Words;
use miyu_store::human::Human;
use miyu_store::presets::Found;

use crate::Core;
use crate::config::methods::words;
use crate::personas::pick;

/// 一个个功能：
/// - 装了的：`{"id", "name", "summary", "on", "installed": true, "tools"}`，名字、说明照它的清单、照语言挑，没说明的没有
///   `summary`；`on` 是叠好以后开不开；`tools` 是归它的、工具目录里现在有的工具，照名字排，每件
///   `{"name", "label", "on"}`：`label` 照给人看的字里这件工具的显示名，没有的照工具名；功能关着的都是关着，开着的照
///   `[tools]` 关没关。
/// - 预设的 `[features]`、`[software]` 里写了、这台机器上没装的：`{"id", "name", "on", "installed": false, "tools": []}`，
///   照编号排；名字照给人看的字 `software/<编号>`（以前内置的几样），没有的是编号。写的是装了的包的编号的不另列（它的功能
///   已经列了）。
pub(super) fn listed(core: &Core, found: &Found, language: &str) -> Vec<Value> {
    let words = words(core, language).ok();
    let catalog = core.tools();
    let found_packages = core.packages();
    let table = crate::packages::features(&found_packages);
    let file = &found.file;
    let mut listed = Vec::new();
    let mut packages = BTreeSet::new();
    for (package, manifest) in crate::packages::manifests(&found_packages) {
        packages.insert(package);
        for feature in manifest.features_of(package) {
            let on = file.opens_in(&feature.id, package);
            let tools: Vec<Value> = catalog
                .specs()
                .map(|spec| spec.name.as_str())
                .filter(|name| {
                    catalog.package_of(name) == Some(package)
                        && table.of_tool(package, name) == Some(feature.id.as_str())
                })
                .map(|name| {
                    json!({
                        "name": name,
                        "label": label(words.as_ref(), name),
                        "on": on && !file.tools_off.contains(name),
                    })
                })
                .collect();
            let mut one = json!({
                "id": feature.id,
                "name": pick(&feature.name, language),
                "on": on,
                "installed": true,
                "tools": tools,
            });
            if let Some(summary) = pick(&feature.summary, language) {
                one["summary"] = json!(summary);
            }
            listed.push(one);
        }
    }
    let written: BTreeSet<&String> = file
        .features
        .keys()
        .chain(file.software.keys())
        .filter(|id| !table.installed(id) && !packages.contains(id.as_str()))
        .collect();
    for id in written {
        let name = words
            .as_ref()
            .and_then(|words| Words::sentence(words, &format!("software/{id}"), &[]))
            .unwrap_or_else(|| id.clone());
        listed.push(json!({
            "id": id,
            "name": name,
            "on": file.opens(id),
            "installed": false,
            "tools": [],
        }));
    }
    listed
}

/// 工具 `name` 给人看的显示名：给人看的字里有的照它，没有的照工具名。
fn label(words: Option<&Human>, name: &str) -> String {
    words
        .and_then(|words| words.tool(name))
        .map_or_else(|| name.to_string(), |face| face.name.clone())
}
