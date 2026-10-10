//! 预设（施工 P-2 上，`docs/blueprint/presets.md`）：造会话时照「开会话时指定、个人设置、系统配置」找预设、几层叠好；
//! `preset.list`、`preset.get`。找哪几层、怎么叠在 `miyu_store::presets`。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_policy::preset::{Chosen, Problem};
use miyu_session::PresetPlaces;
use miyu_store::human::Human;
use miyu_store::presets::{Found, PresetError, Presets};

use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::personas::{label, remove};
use crate::refusal::Refusal;
use crate::settings::PresetSettings;

mod features;
pub(crate) mod write;

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

/// 交给会话的预设的几层和装了的功能（施工 P-2 下；施工 F-3 上起是功能）：改了预设的文件，开着的会话下一个回合换上。
pub(crate) fn places(core: &Core) -> PresetPlaces {
    PresetPlaces {
        presets: presets(core),
        features: crate::packages::features(&core.packages()),
    }
}

/// 找好的预设换成造会话要的：算好装了、没开的功能（施工 F-3 上）。
pub(crate) fn chosen(core: &Core, found: Found) -> Chosen {
    Chosen::new(
        found.id,
        found.file,
        &crate::packages::features(&core.packages()),
    )
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

/// 同 [`refusal`]，写错了的多带一句照 `words` 的语言的 `message` 和第几行（施工 P-3 补）：`preset.get`、`preset.set` 用。
pub(crate) fn told(error: &PresetError, words: Option<&Human>) -> Refusal {
    let refused = refusal(error);
    match (error, words) {
        (PresetError::Invalid(_, _, problem), Some(words)) => {
            refused.telling(sentence(problem, words), problem.line)
        }
        _ => refused,
    }
}

/// 写错了的那一处照 `words` 的语言说（施工 P-3 补；再补起 `preset.list` 也用）。
fn sentence(problem: &Problem, words: &Human) -> Option<String> {
    let key = format!("preset-problems/{}", problem.code.as_str());
    Words::sentence(words, &key, &[("detail", problem.detail.as_str())])
}

/// 列表里写错了的一项（施工 P-3 再补）：`problem` 照 `words` 的语言说，知道第几行的带 `line`，同 `data.message`、`data.line`；
/// 没有字的、读不了的照原话。
fn listed_problem(id: &str, error: &PresetError, words: Option<&Human>) -> Value {
    let mut item = json!({"preset": id, "problem": error.to_string()});
    if let (PresetError::Invalid(_, _, problem), Some(words)) = (error, words) {
        if let Some(message) = sentence(problem, words) {
            item["problem"] = json!(message);
        }
        if let Some(line) = problem.line {
            item["line"] = json!(line);
        }
    }
    item
}

/// `preset.list`：几层里所有的预设，照编号排。每个带名字、说明（照这个连接的语言挑）；写错了的带 `problem`（照这个连接的
/// 语言说，知道第几行的带 `line`，施工 P-3 再补）、不带名字和说明。来自哪几层不往外给（施工 P-3 补）。
pub(crate) async fn list(core: &Core, peer: Peer) -> Result<Value, Refusal> {
    let said = words(core, peer.language).ok();
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
                "name": label(found.file.name.as_ref(), peer.language),
                "summary": label(found.file.summary.as_ref(), peer.language),
                "icon": found.file.icon,
            }),
            Err(error) => listed_problem(&id, &error, said.as_ref()),
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

/// `preset.get`：叠好的样子（施工 P-3 补：只给人要看的）。名字、说明照这个连接的语言挑；`unlisted` 是叠好以后的（几层都没写的
/// 是 `on`）；`features` 是一个个功能，各带下面的工具（施工 F-3 下，`features.rs`）；删了会怎样（`remove`，同人格）。来自哪几层
/// 不往外给。
pub(crate) async fn get(core: &Core, peer: Peer, params: GetParams) -> Result<Value, Refusal> {
    let said = words(core, peer.language).ok();
    let presets = presets(core);
    let found = tokio::task::spawn_blocking(move || presets.find(&params.preset))
        .await
        .map_err(|_| Refusal::INTERNAL)?
        .map_err(|error| told(&error, said.as_ref()))?;
    Ok(describe(core, &found, peer.language))
}

/// 叠好的一个预设照 `preset.get` 写（`preset.set` 的回应也是它，施工 P-3 中），名字、说明照语言 `language` 挑。
fn describe(core: &Core, found: &Found, language: &str) -> Value {
    json!({
        "preset": found.id,
        "name": label(found.file.name.as_ref(), language),
        "summary": label(found.file.summary.as_ref(), language),
        "icon": found.file.icon,
        "unlisted": found.file.unlisted().as_str(),
        "features": features::listed(core, found, language),
        "remove": remove(&found.layers),
    })
}
