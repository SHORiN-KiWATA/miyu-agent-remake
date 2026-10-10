//! 扩展的四个方法（施工 9-4 上，`docs/blueprint/extensions.md`「对外的样子」）：`extension.status` 列起来时读到的 `process` 包，
//! `extension.enable`、`extension.disable` 先写开关再动进程，`extension.restart` 请它退出、重新拉起。开、关、重启一件件办。扩展
//! 自己调不了（`connection.rs` 先拒）。要的能力还有没批的，`enable` 要带上 `approve` 批、`restart` 拒绝（施工 9-4 下上，
//! `approval.rs`）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::{Manifest, PackageKind};
use miyu_store::extensions::{self as file, Switches};

use super::approval::{self, needs_approval};
use super::{State, TARGET, on, processes, read_switches, stderr, switches_path};
use crate::Core;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;

/// `disable`、`restart` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackageParams {
    /// 包的编号。
    package: String,
}

/// `enable` 的参数（施工 9-4 下上）：多一个 `approve`，这一下批哪几个能力。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EnableParams {
    /// 包的编号。
    package: String,
    /// 批的能力的名字：可以不写。
    #[serde(default)]
    approve: Option<Vec<String>>,
}

/// `extension.status`：起来时读到的 `process` 包，照编号排。订阅扩展的推送时的回应也是它（施工 9-4 补）。
pub(crate) fn status(core: &Core, peer: Peer) -> Value {
    let (switches, _) = read_switches(core);
    let packages = core.packages();
    let listed: Vec<Value> = processes(&packages)
        .map(|(id, manifest)| one(core, id, manifest, &switches, peer))
        .collect();
    json!({ "extensions": listed })
}

/// 包 `id` 这时的一项（施工 9-4 补，`extension.changed` 推的就是它）：不是起来时读到的 `process` 包的没有。
pub(crate) fn entry(core: &Core, id: &str, peer: Peer) -> Option<Value> {
    let manifest = &extension(core, id).ok()?;
    let (switches, _) = read_switches(core);
    Some(one(core, id, manifest, &switches, peer))
}

/// `extension.enable`：要的能力还有没批的，照 `approve` 批（施工 9-4 下上）；批过的、开着的一起记下，照登记缓存先登记
/// 它的工具（施工 O-2 中），没在跑的拉起。
pub(crate) async fn enable(
    core: &Arc<Core>,
    peer: Peer,
    params: EnableParams,
) -> Result<Value, Refusal> {
    turn_on(core, peer, &params.package, params.approve.as_deref()).await
}

/// 打开扩展 `id`（`extension.enable`；施工 F-6 上起 `package.enable` 也走这里）：程序不在的拒绝 `program_missing`，开了也起不来
/// （`package-pages.md`「程序不在就当没装」）。
pub(crate) async fn turn_on(
    core: &Arc<Core>,
    peer: Peer,
    id: &str,
    approve: Option<&[String]>,
) -> Result<Value, Refusal> {
    let manifest = &extension(core, id)?;
    if crate::packages::status::program_missing(manifest) {
        return Err(Refusal::PROGRAM_MISSING);
    }
    let _one_at_a_time = core.extensions.ops.lock().await;
    let (mut switches, version) = read_switches(core);
    approval::approve(core, id, manifest, &mut switches, approve)?;
    switches.on.insert(id.to_string(), true);
    write(core, id, &switches, version.as_deref())?;
    crate::provide::restore(core, id);
    core.extensions.launch(core, id, manifest);
    core.extensions.notify(id);
    Ok(one(core, id, manifest, &switches, peer))
}

/// `extension.disable`：记成关着，在跑的请它退出、等它退出，它的工具出目录（施工 O-2 中）。
pub(crate) async fn disable(
    core: &Arc<Core>,
    peer: Peer,
    params: PackageParams,
) -> Result<Value, Refusal> {
    turn_off(core, peer, &params.package).await
}

/// 关掉扩展 `id`（`extension.disable`；施工 F-6 上起 `package.disable` 也走这里）。
pub(crate) async fn turn_off(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    let manifest = &extension(core, id)?;
    let _one_at_a_time = core.extensions.ops.lock().await;
    let (mut switches, version) = read_switches(core);
    switches.on.insert(id.to_string(), false);
    write(core, id, &switches, version.as_deref())?;
    core.extensions.halt(id).await;
    crate::provide::withdraw(core, id);
    core.extensions.notify(id);
    Ok(one(core, id, manifest, &switches, peer))
}

/// `extension.restart`：请它退出、等它退出，重新拉起，连续失败从零数。关着的拒绝；要的能力还有没批的拒绝
/// `needs_approval`（批走 `enable`，施工 9-4 下上）。
pub(crate) async fn restart(
    core: &Arc<Core>,
    peer: Peer,
    params: PackageParams,
) -> Result<Value, Refusal> {
    let manifest = &extension(core, &params.package)?;
    let _one_at_a_time = core.extensions.ops.lock().await;
    let (switches, _) = read_switches(core);
    if !on(&switches, &params.package, manifest) {
        return Err(Refusal::EXTENSION_OFF);
    }
    let left = approval::unapproved(core, &params.package, manifest, &switches);
    if !left.is_empty() {
        return Err(needs_approval(&left));
    }
    core.extensions.halt(&params.package).await;
    core.extensions.launch(core, &params.package, manifest);
    Ok(one(core, &params.package, manifest, &switches, peer))
}

/// 编号是 `id` 的 `process` 包的清单：没有、读不成的 `unknown_package`，是界面的 `not_an_extension`。
fn extension(core: &Core, id: &str) -> Result<Manifest, Refusal> {
    let manifest = core
        .packages()
        .iter()
        .find(|found| found.id == id)
        .and_then(|found| found.read.as_ref().ok())
        .cloned()
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    match manifest.kind {
        PackageKind::Process => Ok(manifest),
        PackageKind::Ui | PackageKind::Builtin | PackageKind::Worker | PackageKind::Mascot => {
            Err(Refusal::NOT_AN_EXTENSION)
        }
    }
}

/// 把改过的开关（开、关、批过的能力）整份写回：写不进的拒绝（`internal_error`），进程不动。`version` 是读的时候的版本。
fn write(core: &Core, id: &str, switches: &Switches, version: Option<&str>) -> Result<(), Refusal> {
    file::write(&switches_path(core), switches, version).map_err(|error| {
        tracing::error!(target: TARGET, package = id, error = %error, "extension switch not written");
        Refusal::INTERNAL
    })
}

/// 一个扩展：编号、名字（照连接的语言挑）、`start`、开没开、这时的状态；声明了能力的多 `capabilities`，还有没批的多
/// `unapproved`（施工 9-4 下上）。开没开、批过哪些照 `switches`。
fn one(core: &Core, id: &str, manifest: &Manifest, switches: &Switches, peer: Peer) -> Value {
    let on = super::on(switches, id, manifest);
    let status = core.extensions.status(id);
    let start = manifest
        .process
        .as_ref()
        .map_or("manual", |process| process.start.as_str());
    let mut item = json!({
        "package": id,
        "name": pick(&manifest.name, peer.language),
        "start": start,
        "on": on,
        "state": status.state.as_str(),
        "failures": status.failures,
    });
    match status.state {
        State::Starting { pid: Some(pid) } | State::Running { pid } => item["pid"] = json!(pid),
        State::Waiting { until } => {
            let left = until.saturating_duration_since(tokio::time::Instant::now());
            item["retry_in"] = json!(u64::try_from(left.as_millis()).unwrap_or(u64::MAX));
        }
        State::Stopped(reason) => {
            item["reason"] = json!(reason.as_str());
            if let Some(tail) = stderr::tail(&stderr::path(core, id)) {
                item["stderr"] = json!(tail);
            }
        }
        State::Starting { pid: None } | State::Off => {}
    }
    if let Some((listed, left)) = approval::shown(core, id, manifest, switches, peer.language) {
        item["capabilities"] = listed;
        if let Some(left) = left {
            item["unapproved"] = left;
        }
    }
    item
}
