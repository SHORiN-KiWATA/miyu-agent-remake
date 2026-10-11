//! 装、卸之前看一眼（施工 F-8 下补，设计 `31-软件包.md` 第一节、定了的 B）：`package.install`、`package.remove` 带
//! `preview: true` 的，照真做之前查的那几样查一遍（写错的、撞了的、必需的照样拒），交回要做什么，什么都不动。命令行照它印一份、
//! 问「继续？[Y/n]」（`cli/pkg.md`）。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Map, Value, json};

use miyu_config::package::{Manifest, PackageKind};
use miyu_store::packages::local;
use miyu_store::packages::{Layer, Packages};

use super::manage::{removable, setting_keys, source};
use super::packages;
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 从包文件夹 `path` 装：编号、版本、换下哪一份（同编号已经装了的）、带了什么、要什么能力、几个文件多大。
pub(crate) async fn installing(
    core: &Arc<Core>,
    peer: Peer,
    path: &Path,
) -> Result<Value, Refusal> {
    let (folder, id, manifest) = source(core, peer, path)?;
    let files = tokio::task::spawn_blocking(move || local::scan(&folder))
        .await
        .map_err(|_| Refusal::INTERNAL)?
        .map_err(|_| Refusal::PATH_UNREADABLE)?;
    let mut answer = carries(core, peer, &id, &manifest);
    let now = core.packages();
    if let Some(old) = now.iter().find(|one| one.id == id) {
        let version = old.read.as_ref().ok().and_then(|old| old.version.clone());
        answer.insert("replaces".into(), json!({ "version": version }));
    }
    answer.insert("files".into(), json!(files.len()));
    answer.insert("size".into(), json!(local::size(&files)));
    Ok(Value::Object(answer))
}

/// 把卸掉的出厂的包装回来：编号、版本、带了什么，`restores: true`。没卸过的 `unknown_package`。
pub(crate) fn restoring(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    if !packages(core).removed().iter().any(|removed| removed == id) {
        return Err(Refusal::UNKNOWN_PACKAGE);
    }
    let shipped = Packages::shipped(&core.resources).read();
    let manifest = shipped
        .iter()
        .find(|one| one.id == id)
        .and_then(|one| one.read.as_ref().ok())
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let mut answer = carries(core, peer, id, manifest);
    answer.insert("restores".into(), json!(true));
    Ok(Value::Object(answer))
}

/// 卸：编号、哪一层、版本；家目录里的几个文件多大；一并删掉的配置项（系统配置、个人设置里写了的键）、有没有状态目录。
pub(crate) async fn removing(core: &Arc<Core>, id: &str) -> Result<Value, Refusal> {
    let found = core.packages();
    let one = removable(&found, id)?;
    let mut answer = Map::new();
    answer.insert("package".into(), json!(id));
    answer.insert("layer".into(), json!(one.layer.as_str()));
    let version = one.read.as_ref().ok().and_then(|it| it.version.clone());
    if let Some(version) = version {
        answer.insert("version".into(), json!(version));
    }
    if one.layer == Layer::Home {
        let looked = super::local::entry(core, id).await?;
        answer.insert("files".into(), json!(looked.files.len()));
        answer.insert("size".into(), json!(local::size(&looked.files)));
    }
    let settings = crate::config::forget::present(core, &setting_keys(one));
    answer.insert("settings".into(), json!(settings));
    let state = packages(core).state_dir(id);
    let state = tokio::task::spawn_blocking(move || state.is_dir())
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    answer.insert("state".into(), json!(state));
    Ok(Value::Object(answer))
}

/// 一份清单带了什么：编号、版本（写了的）、程序（`ui`、`process`、`worker`、`builtin`，只带吉祥物的没有）、子命令名、后台页、
/// 吉祥物、接哪个平台、系统账号、几个配置项、声明的能力（照连接的语言说，`extension.status` 那一格的样子）。没有的不写。
fn carries(core: &Core, peer: Peer, id: &str, manifest: &Manifest) -> Map<String, Value> {
    let mut answer = Map::new();
    answer.insert("package".into(), json!(id));
    if let Some(version) = &manifest.version {
        answer.insert("version".into(), json!(version));
    }
    if manifest.kind != PackageKind::Mascot {
        answer.insert("program".into(), json!(manifest.kind.as_str()));
    }
    if let Some(command) = &manifest.command {
        answer.insert("command".into(), json!(command.name));
    }
    if manifest.page.is_some() {
        answer.insert("page".into(), json!(true));
    }
    if manifest.mascot.is_some() {
        answer.insert("mascot".into(), json!(true));
    }
    if let Some(connection) = &manifest.connection {
        answer.insert("connection".into(), json!(connection.platform));
    }
    if manifest
        .process
        .as_ref()
        .is_some_and(|process| process.system_account)
    {
        answer.insert("system_account".into(), json!(true));
    }
    if !manifest.settings.is_empty() {
        answer.insert("settings".into(), json!(manifest.settings.len()));
    }
    if let Some(listed) = crate::extensions::described(core, manifest, peer.language) {
        answer.insert("capabilities".into(), listed);
    }
    answer
}
