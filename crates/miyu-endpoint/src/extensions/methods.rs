//! 扩展的四个方法（施工 9-4 上，`docs/blueprint/extensions.md`「对外的样子」）：`extension.status` 列起来时读到的 `process` 包，
//! `extension.enable`、`extension.disable` 先写开关再动进程，`extension.restart` 请它退出、重新拉起。开、关、重启一件件办。扩展
//! 自己调不了（`connection.rs` 先拒）。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::{Manifest, PackageKind};
use miyu_store::extensions as file;

use super::{State, TARGET, on, processes, read_switches, stderr, switches_path};
use crate::Core;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;

/// 三个改动的方法的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackageParams {
    /// 包的编号。
    package: String,
}

/// `extension.status`：起来时读到的 `process` 包，照编号排。订阅扩展的推送时的回应也是它（施工 9-4 补）。
pub(crate) fn status(core: &Core, peer: Peer) -> Value {
    let (switches, _) = read_switches(core);
    let listed: Vec<Value> = processes(core)
        .map(|(id, manifest)| one(core, id, manifest, on(&switches, id, manifest), peer))
        .collect();
    json!({ "extensions": listed })
}

/// 包 `id` 这时的一项（施工 9-4 补，`extension.changed` 推的就是它）：不是起来时读到的 `process` 包的没有。
pub(crate) fn entry(core: &Core, id: &str, peer: Peer) -> Option<Value> {
    let manifest = extension(core, id).ok()?;
    let (switches, _) = read_switches(core);
    Some(one(core, id, manifest, on(&switches, id, manifest), peer))
}

/// `extension.enable`：记成开着，没在跑的拉起。
pub(crate) async fn enable(
    core: &Arc<Core>,
    peer: Peer,
    params: PackageParams,
) -> Result<Value, Refusal> {
    let manifest = extension(core, &params.package)?;
    let _one_at_a_time = core.extensions.ops.lock().await;
    switch(core, &params.package, true)?;
    core.extensions.launch(core, &params.package, manifest);
    core.extensions.notify(&params.package);
    Ok(one(core, &params.package, manifest, true, peer))
}

/// `extension.disable`：记成关着，在跑的请它退出、等它退出。
pub(crate) async fn disable(
    core: &Arc<Core>,
    peer: Peer,
    params: PackageParams,
) -> Result<Value, Refusal> {
    let manifest = extension(core, &params.package)?;
    let _one_at_a_time = core.extensions.ops.lock().await;
    switch(core, &params.package, false)?;
    core.extensions.halt(&params.package).await;
    core.extensions.notify(&params.package);
    Ok(one(core, &params.package, manifest, false, peer))
}

/// `extension.restart`：请它退出、等它退出，重新拉起，连续失败从零数。关着的拒绝。
pub(crate) async fn restart(
    core: &Arc<Core>,
    peer: Peer,
    params: PackageParams,
) -> Result<Value, Refusal> {
    let manifest = extension(core, &params.package)?;
    let _one_at_a_time = core.extensions.ops.lock().await;
    let (switches, _) = read_switches(core);
    if !on(&switches, &params.package, manifest) {
        return Err(Refusal::EXTENSION_OFF);
    }
    core.extensions.halt(&params.package).await;
    core.extensions.launch(core, &params.package, manifest);
    Ok(one(core, &params.package, manifest, true, peer))
}

/// 编号是 `id` 的 `process` 包的清单：没有、读不成的 `unknown_package`，是界面的 `not_an_extension`。
fn extension<'a>(core: &'a Core, id: &str) -> Result<&'a Manifest, Refusal> {
    let manifest = core
        .packages
        .iter()
        .find(|found| found.id == id)
        .and_then(|found| found.read.as_ref().ok())
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    match manifest.kind {
        PackageKind::Process => Ok(manifest),
        PackageKind::Ui => Err(Refusal::NOT_AN_EXTENSION),
    }
}

/// 把包 `id` 记成开着、关着：写不进的拒绝（`internal_error`），进程不动。
fn switch(core: &Core, id: &str, value: bool) -> Result<(), Refusal> {
    let (mut switches, version) = read_switches(core);
    switches.on.insert(id.to_string(), value);
    file::write(&switches_path(core), &switches, version.as_deref()).map_err(|error| {
        tracing::error!(target: TARGET, package = id, error = %error, "extension switch not written");
        Refusal::INTERNAL
    })
}

/// 一个扩展：编号、名字（照连接的语言挑）、`start`、开没开、这时的状态。
fn one(core: &Core, id: &str, manifest: &Manifest, on: bool, peer: Peer) -> Value {
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
    item
}
