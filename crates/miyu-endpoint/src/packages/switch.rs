//! 列表上的开关（施工 F-6 上，`docs/blueprint/package-pages.md`「开关」）：头只发 `package.enable`、`package.disable`，怎么做照
//! 种类定。扩展开关进程（同 `extension.enable`、`extension.disable`）；出厂的、不是必需的内置包关是卸掉（在家目录记一笔）、
//! 开是装回来；必需的、界面、小程序没有开关。回应同 `package.list` 的一项。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use miyu_config::package::{Manifest, PackageKind};
use miyu_store::packages::Layer;

use super::{listed, manage, packages};
use crate::Core;
use crate::config::methods::words;
use crate::extensions::methods::{turn_off, turn_on};
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `package.enable` 的参数：`approve` 同 `extension.enable`，批扩展要的能力。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EnableParams {
    package: String,
    #[serde(default)]
    approve: Option<Vec<String>>,
}

/// `package.disable` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DisableParams {
    package: String,
}

/// 开关怎么做。
enum Switch {
    /// 扩展：开关进程。
    Extension,
    /// 出厂的、不是必需的内置包：卸掉、装回来。
    Shipped,
}

/// 包 `id` 现在的清单、是不是卸掉了的出厂包；没有的 `unknown_package`。
fn find(core: &Core, id: &str) -> Result<(Manifest, Layer, bool), Refusal> {
    if let Some(found) = core.packages().iter().find(|found| found.id == id) {
        let manifest = found
            .read
            .as_ref()
            .map_err(|_| Refusal::UNKNOWN_PACKAGE)?
            .clone();
        return Ok((manifest, found.layer, false));
    }
    let removed = packages(core).read_removed();
    let found = removed
        .iter()
        .find(|found| found.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let manifest = found
        .read
        .as_ref()
        .map_err(|_| Refusal::UNKNOWN_PACKAGE)?
        .clone();
    Ok((manifest, found.layer, true))
}

/// 这个包的开关怎么做：必需的 `package_required`，没有开关的 `not_switchable`。
fn switch(manifest: &Manifest, layer: Layer) -> Result<Switch, Refusal> {
    if manifest.required {
        return Err(Refusal::PACKAGE_REQUIRED);
    }
    match manifest.kind {
        PackageKind::Process => Ok(Switch::Extension),
        PackageKind::Builtin if layer == Layer::Shipped => Ok(Switch::Shipped),
        PackageKind::Builtin | PackageKind::Ui | PackageKind::Worker | PackageKind::Mascot => {
            Err(Refusal::NOT_SWITCHABLE)
        }
    }
}

/// `package.enable`。卸掉了的出厂包先装回来；扩展再照 `approve` 打开。
pub(crate) async fn enable(
    core: &Arc<Core>,
    peer: Peer,
    params: EnableParams,
) -> Result<Value, Refusal> {
    let (manifest, layer, removed) = find(core, &params.package)?;
    let how = switch(&manifest, layer)?;
    if crate::packages::status::program_missing(&manifest) {
        return Err(Refusal::PROGRAM_MISSING);
    }
    if removed {
        manage::bring_back(core, peer, &params.package).await?;
    }
    if let Switch::Extension = how {
        turn_on(core, peer, &params.package, params.approve.as_deref()).await?;
    }
    item(core, peer, &params.package)
}

/// `package.disable`。扩展关掉进程；出厂的内置包卸掉。
pub(crate) async fn disable(
    core: &Arc<Core>,
    peer: Peer,
    params: DisableParams,
) -> Result<Value, Refusal> {
    let (manifest, layer, removed) = find(core, &params.package)?;
    let how = switch(&manifest, layer)?;
    if !removed {
        match how {
            Switch::Extension => {
                turn_off(core, peer, &params.package).await?;
            }
            Switch::Shipped => {
                manage::remove_package(core, params.package.clone()).await?;
            }
        }
    }
    item(core, peer, &params.package)
}

/// 包 `id` 这时在 `package.list` 里的那一项（卸掉了的出厂包带 `removed: true`）。
fn item(core: &Core, peer: Peer, id: &str) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let places = packages(core);
    if let Some(found) = core.packages().iter().find(|found| found.id == id) {
        return Ok(listed(core, found, &places, &words, peer.language, false));
    }
    let removed = places.read_removed();
    let found = removed
        .iter()
        .find(|found| found.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    Ok(listed(core, found, &places, &words, peer.language, true))
}
