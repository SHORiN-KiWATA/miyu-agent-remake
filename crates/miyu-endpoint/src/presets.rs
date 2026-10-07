//! 预设（施工 P-2 上，`docs/blueprint/presets.md`）：造会话时照「开会话时指定、个人设置、系统配置」找预设、几层叠好；
//! `preset.list`、`preset.get`。找哪几层、怎么叠在 `miyu_store::presets`。

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::PackageKind;
use miyu_policy::preset::{Chosen, ROLEPLAY};
use miyu_store::presets::{Found, Layer, PresetError, Presets};

use crate::Core;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;
use crate::settings::PresetSettings;

const TARGET: &str = "miyu::endpoint";

/// 这个核心的几层预设：出厂、系统区、管理员的家目录。
pub(crate) fn presets(core: &Core) -> Presets {
    Presets::new(&core.resources, &core.root, &core.admin)
}

/// 找新会话的预设：指定了的是它，不然照这时的 `preset.default`（个人设置压着系统配置，都没写的是出厂的 `full`）。指着没有
/// 的不悄悄换成别的（Y12）。在阻塞线程里读盘。
pub(crate) async fn resolve(core: &Core, wanted: Option<&str>) -> Result<Found, Refusal> {
    let id = match wanted {
        Some(id) => id.to_string(),
        None => PresetSettings::from(&core.config().resolved().values()).default,
    };
    let presets = presets(core);
    let found = tokio::task::spawn_blocking(move || presets.find(&id)).await;
    match found {
        Ok(Ok(found)) => Ok(found),
        Ok(Err(error)) => Err(refusal(&error)),
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// 这台机器上装了的软件（施工 P-2 中，`presets.md`「照预设挑」）：工具目录里有工具的包、角色扮演，和清单装的 `process` 包
/// （桥，读成了的）。界面包是头，不算。
pub(crate) fn installed(core: &Core) -> BTreeSet<String> {
    let mut installed: BTreeSet<String> = core.tools.packages().map(str::to_string).collect();
    installed.insert(ROLEPLAY.to_string());
    installed.extend(core.packages.iter().filter_map(|found| match &found.read {
        Ok(manifest) if manifest.kind == PackageKind::Process => Some(found.id.clone()),
        _ => None,
    }));
    installed
}

/// 找好的预设换成造会话要的：算好装了、没开的那几个。
pub(crate) fn chosen(core: &Core, found: Found) -> Chosen {
    let installed = installed(core);
    Chosen::new(found.id, found.file, installed.iter().map(String::as_str))
}

/// 找预设出的错照协议说：编号不合写法的参数不对，哪一层都没有的 `unknown_preset`，写错了的 `preset_invalid`（`data.problem`
/// 写明哪一层、哪个文件第几行），读不了的是内部出错。
fn refusal(error: &PresetError) -> Refusal {
    match error {
        PresetError::BadId(_) => Refusal::BAD_PARAMS,
        PresetError::NotFound(_) => Refusal::UNKNOWN_PRESET,
        PresetError::Invalid(..) => Refusal::preset_invalid(error.to_string()),
        PresetError::Unreadable(..) => {
            tracing::warn!(target: TARGET, error = %error, "preset unreadable");
            Refusal::INTERNAL
        }
    }
}

/// `preset.list`：几层里所有的预设，照编号排。每个带名字、说明（照这个连接的语言挑）、来自哪几层；写错了的带 `problem`、
/// 不带名字和说明。
pub(crate) async fn list(core: &Core, peer: Peer) -> Result<Value, Refusal> {
    let presets = presets(core);
    let read = tokio::task::spawn_blocking(move || {
        presets
            .ids()
            .into_iter()
            .map(|id| {
                let found = presets.find(&id);
                (id, found)
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let listed: Vec<Value> = read
        .into_iter()
        .map(|(id, found)| match found {
            Ok(found) => json!({
                "preset": id,
                "name": pick(&found.file.name, peer.language),
                "summary": pick(&found.file.summary, peer.language),
                "layers": layers(&found.layers),
            }),
            Err(error) => json!({"preset": id, "problem": error.to_string()}),
        })
        .collect();
    Ok(json!({"presets": listed}))
}

/// `preset.get` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetParams {
    preset: String,
}

/// `preset.get`：叠好的样子。名字、说明的几种语言原样给；默认人格没写的是 `null`；`unlisted` 是叠好以后的（几层都没写的是
/// `on`）；`software` 是包到开不开，`tools` 是关掉的单件工具，都是 `false`；`missing` 是 `[software]` 里写了、这台机器上没装的
/// （施工 P-2 中，照编号排）。
pub(crate) async fn get(core: &Core, params: GetParams) -> Result<Value, Refusal> {
    let found = resolve(core, Some(&params.preset)).await?;
    let installed = installed(core);
    let missing: Vec<&String> = found
        .file
        .software
        .keys()
        .filter(|software| !installed.contains(*software))
        .collect();
    let tools: serde_json::Map<String, Value> = found
        .file
        .tools_off
        .iter()
        .map(|tool| (tool.clone(), json!(false)))
        .collect();
    Ok(json!({
        "preset": found.id,
        "name": found.file.name,
        "summary": found.file.summary,
        "layers": layers(&found.layers),
        "default_persona": found.file.default_persona,
        "unlisted": found.file.unlisted().as_str(),
        "software": found.file.software,
        "tools": tools,
        "missing": missing,
    }))
}

fn layers(layers: &[Layer]) -> Vec<&'static str> {
    layers.iter().map(|layer| layer.as_str()).collect()
}
