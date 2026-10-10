//! 装、卸（施工 F-5 上，设计 `30-插件框架.md` 第九节，`docs/blueprint/packages.md`「装卸」）：`package.install`、
//! `package.remove`。只动管理员家目录那一层（磁盘上怎么做在 `miyu_store::packages::install`），做完照两层重读、当场换掉核心
//! 手里的清单（[`Core::reload_packages`]）：`package.list`、预设的功能、开着的会话下一个回合都照新的。一次只做一件。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package;
use miyu_policy::preset::MEMORY;
use miyu_store::packages::install::{self, Placed};
use miyu_store::packages::{Found, Issue, Layer, MANIFEST, Packages};

use super::{TARGET, compiled, listed, load, packages, sentence, settle};
use crate::Core;
use crate::config::methods::words;
use crate::extensions::processes_of;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `package.install` 的参数：从一份清单装（`path`），或者把卸掉的出厂的包装回来（`package`），二者写一个。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstallParams {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    package: Option<String>,
}

/// `package.remove` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveParams {
    package: String,
}

impl Core {
    /// 照两层重读软件包清单，标没编进来的内置包、认配置项撞没撞，当场换掉核心手里的那一份（施工 F-5 上）。整份配置清单照
    /// 端口拼、配置服务换上（施工 F-5 补）；没设端口的（测试里造的核心）照手里的清单认撞没撞，配置清单不换。
    pub(crate) fn reload_packages(&self) {
        let mut found = load(&self.resources, &self.root, &self.admin);
        if let Some(built_in) = &self.built_in {
            compiled(&mut found, built_in);
        }
        let items = match &self.builtins {
            Some(builtins) => Some(builtins.settings(&mut found)),
            None => {
                let items = self.config().items().to_vec();
                let _ = settle(&mut found, &items);
                None
            }
        };
        self.rebuild_builtins(&found);
        *self
            .packages
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = std::sync::Arc::new(found);
        if let Some(items) = items {
            crate::config::refit::refit(self, items);
        }
    }

    /// 装卸时照清单从 `before` 换到 `now`（施工 F-5 下、补、再补）：人格记忆装没装照 `now` 设（`Memory::set_installed`，施工
    /// R-10：开着的会话照它交不交摘要、抽不抽），扩展进程跟着停下、拉起（[`Core::follow_packages`]），本机的向量模型照 `now`
    /// 换（`Vectors::replace_local`，一样的不动，旧的小程序退完才返回）。卸包、升级在动文件以前照去掉它的清单换一遍：Windows
    /// 上开着的文件删不掉、挪不走。
    async fn switch_packages(self: &Arc<Self>, before: &[&Found], now: &[&Found]) {
        let memory = now
            .iter()
            .any(|one| super::is_installed(std::slice::from_ref(*one), MEMORY));
        self.memory.set_installed(memory);
        self.follow_packages(&processes_of(before), &processes_of(now))
            .await;
        if let (Some(builtins), Some(vectors)) = (&self.builtins, self.memory.vectors()) {
            vectors.replace_local(builtins.embed(now)).await;
        }
    }

    /// 照清单 `found` 换内置包的工具（施工 F-5 中）：新装上的换进去，卸掉的拿掉、记下随包卸掉了（用过它的会话调到时报「已
    /// 卸载」）。没设端口、不知道编进来了哪些的不动。换不成的记一行、照旧。
    fn rebuild_builtins(&self, found: &[Found]) {
        let (Some(builtins), Some(built_in)) = (&self.builtins, &self.built_in) else {
            return;
        };
        let groups = match builtins.tools(found) {
            Ok(groups) => groups,
            Err(error) => {
                tracing::warn!(target: TARGET, error = error.as_str(), "builtin tools not rebuilt");
                return;
            }
        };
        let current = self.tools.current();
        let moving = built_in.iter().any(|package| {
            let has = current.packages().any(|owner| owner == *package);
            let wanted = groups.iter().any(|(id, _)| id == package);
            has != wanted
        });
        if !moving {
            return;
        }
        let changed = self.tools.replace(|catalog| {
            let mut next = catalog.clone();
            for package in built_in {
                let has = catalog.packages().any(|owner| owner == *package);
                let wanted = groups.iter().find(|(id, _)| id == package);
                next = match (wanted, has) {
                    (Some((_, tools)), false) => next.placing(package, tools.clone())?,
                    (None, true) => next.removing(package),
                    _ => next,
                };
            }
            Ok::<_, miyu_tool::CatalogError>(next)
        });
        if let Err(error) = changed {
            tracing::warn!(target: TARGET, error = %error, "builtin tools not rebuilt");
        }
    }
}

impl Core {
    /// 登记的查询 `method` 这时答不答（施工 F-5 中）：属于哪个软件包的，那个包这时装着才答；不属于哪个包的照答。
    pub(crate) fn serves(&self, method: &str) -> bool {
        self.queries
            .package_of(method)
            .is_none_or(|package| super::is_installed(&self.packages(), package))
    }
}

/// `package.install`。
pub(crate) async fn install(
    core: &Arc<Core>,
    peer: Peer,
    params: InstallParams,
) -> Result<Value, Refusal> {
    let _one_at_a_time = core.packaging.lock().await;
    match (params.path, params.package) {
        (Some(path), None) => from_path(core, peer, Path::new(&path)).await,
        (None, Some(id)) => bring_back(core, peer, &id).await,
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// `package.remove`：家目录那一层的删掉；出厂的记一笔；必需的拒绝。
pub(crate) async fn remove(core: &Arc<Core>, params: RemoveParams) -> Result<Value, Refusal> {
    remove_package(core, params.package).await
}

/// 卸包 `id`（`package.remove`，施工 F-6 上起 `package.disable` 关出厂的内置包也走这里）。
pub(super) async fn remove_package(core: &Arc<Core>, id: String) -> Result<Value, Refusal> {
    let _one_at_a_time = core.packaging.lock().await;
    if !miyu_store::personas::valid(&id) {
        return Err(Refusal::BAD_PARAMS);
    }
    let found = core.packages();
    let one = found
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    if one.read.as_ref().is_ok_and(|manifest| manifest.required) {
        return Err(Refusal::PACKAGE_REQUIRED);
    }
    let home = home(core)?;
    let local_root = packages(core).local_root();
    let state = packages(core).state_dir(&id);
    // 卸就是清干净（施工 F-8 中下补，设计 31 定了的 H）：它的配置项照清单列的删，状态目录整个删。
    let keys: Vec<String> = one
        .read
        .as_ref()
        .map(|manifest| {
            manifest
                .settings
                .iter()
                .map(|setting| format!("{id}.{}", setting.name))
                .collect()
        })
        .unwrap_or_default();
    let layer = one.layer;
    // 先停下用着它的、再删文件（施工 F-5 补）：Windows 上开着的文件删不掉。删不成的照原来的清单换回来。
    let all = refs(&found);
    let without = leaving(&all, &id);
    core.switch_packages(&all, &without).await;
    let removed = blocking(move || match layer {
        Layer::Home => install::take_out(&home, &id).map(|()| {
            super::local::forget(&local_root, &id);
            id
        }),
        Layer::Shipped => install::mark_removed(&home, &id).map(|()| id),
    })
    .await;
    let removed = match removed {
        Ok(removed) => removed,
        Err(refusal) => {
            core.switch_packages(&without, &all).await;
            return Err(refusal);
        }
    };
    core.reload_packages();
    let now = core.packages();
    core.switch_packages(&without, &refs(&now)).await;
    crate::config::forget::forget(core, &keys);
    if let Err(refusal) = blocking(move || remove_dir_if_there(&state)).await {
        tracing::warn!(target: TARGET, package = removed.as_str(), reason = refusal.reason, "package state not removed");
    }
    tracing::info!(target: TARGET, package = removed.as_str(), "package removed");
    Ok(json!({"package": removed, "removed": true}))
}

/// 从 `path` 这个包文件夹装（施工 F-8 上：一个文件夹就是一个包；写成文件夹里的 `package.toml` 也认）：读得成、不和出厂的撞、
/// 拷进去以后和别的包也不撞，才算装上。编号是文件夹的名字。
async fn from_path(core: &Arc<Core>, peer: Peer, path: &Path) -> Result<Value, Refusal> {
    let folder = match path.file_name().and_then(|name| name.to_str()) {
        Some(MANIFEST) => path.parent().ok_or(Refusal::BAD_PARAMS)?,
        _ => path,
    };
    let id = folder
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|id| miyu_store::personas::valid(id))
        .map(str::to_string)
        .filter(|_| folder.is_absolute())
        .ok_or(Refusal::BAD_PARAMS)?;
    let text =
        std::fs::read_to_string(folder.join(MANIFEST)).map_err(|_| Refusal::PATH_UNREADABLE)?;
    let words = words(core, peer.language)?;
    if let Err(problem) = package::read(&text) {
        return Err(invalid(&words, &problem));
    }
    let places = packages(core);
    let shipped = Packages::shipped(&core.resources).read();
    if shipped.iter().any(|one| one.id == id) {
        return Err(Refusal::PACKAGE_EXISTS);
    }
    let home = home(core)?;
    let source = folder.to_path_buf();
    let target = id.clone();
    // 升级的先停下原来的那一个再换文件（施工 F-5 补）：Windows 上开着的文件挪不走。换不成的照原来的清单换回来。
    let before = core.packages();
    let all = refs(&before);
    let without = leaving(&all, &id);
    core.switch_packages(&all, &without).await;
    let placed = blocking(move || install::place(&home, &target, &source)).await;
    let placed: Placed = match placed {
        Ok(placed) => placed,
        Err(refusal) => {
            core.switch_packages(&without, &all).await;
            return Err(refusal);
        }
    };
    core.reload_packages();
    let now = core.packages();
    let mine = now.iter().find(|one| one.id == id);
    if let Some(Found {
        read: Err(Issue::Wrong(problem)),
        ..
    }) = mine
    {
        let refusal = invalid(&words, problem);
        if let Err(error) = placed.undo() {
            tracing::warn!(target: TARGET, package = id.as_str(), error = %error, "package install not undone");
        }
        core.reload_packages();
        let back = core.packages();
        core.switch_packages(&without, &refs(&back)).await;
        return Err(refusal);
    }
    placed.keep();
    // 本地库记下这一份（施工 F-8 中上）：记不成的记一行，装成了照算。
    let version = mine
        .and_then(|one| one.read.as_ref().ok())
        .and_then(|manifest| manifest.version.clone());
    let (record_places, record_id, from) =
        (places.clone(), id.clone(), folder.display().to_string());
    if let Err(refusal) =
        blocking(move || super::local::record(&record_places, &record_id, version, &from)).await
    {
        tracing::warn!(target: TARGET, package = id.as_str(), reason = refusal.reason, "package not recorded");
    }
    core.switch_packages(&without, &refs(&now)).await;
    tracing::info!(target: TARGET, package = id.as_str(), "package installed");
    let mine = now
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::INTERNAL)?;
    Ok(listed(core, mine, &places, &words, peer.language, false))
}

/// 把卸掉的出厂的包 `id` 装回来：删掉家目录里记的那一笔。
pub(super) async fn bring_back(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    let places = packages(core);
    if !places.removed().iter().any(|removed| removed == id) {
        return Err(Refusal::UNKNOWN_PACKAGE);
    }
    let home = home(core)?;
    let target = id.to_string();
    let before = core.packages();
    blocking(move || install::unmark_removed(&home, &target)).await?;
    core.reload_packages();
    let now = core.packages();
    core.switch_packages(&refs(&before), &refs(&now)).await;
    tracing::info!(target: TARGET, package = id, "package restored");
    let words = words(core, peer.language)?;
    let now = core.packages();
    let back = now
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    Ok(listed(core, back, &places, &words, peer.language, false))
}

/// 管理员家目录那一层。
fn home(core: &Core) -> Result<PathBuf, Refusal> {
    packages(core)
        .home_dir()
        .map(Path::to_path_buf)
        .ok_or(Refusal::INTERNAL)
}

/// 清单写错了、撞了：照连接的语言说一句。
fn invalid(words: &miyu_store::human::Human, problem: &package::Problem) -> Refusal {
    let said = sentence(words, problem.code.as_str(), &problem.detail)
        .unwrap_or_else(|| problem.message.clone());
    Refusal::package_invalid(said, problem.line)
}

/// 在阻塞线程里动磁盘；动不了的记一行、算内部出错。
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> std::io::Result<T> + Send + 'static,
) -> Result<T, Refusal> {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(done)) => Ok(done),
        Ok(Err(error)) => {
            tracing::warn!(target: TARGET, error = %error, "package files not changed");
            Err(Refusal::INTERNAL)
        }
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// 清单里的每一份。
fn refs(found: &[Found]) -> Vec<&Found> {
    found.iter().collect()
}

/// 去掉编号是 `id` 的那一份。
fn leaving<'a>(all: &[&'a Found], id: &str) -> Vec<&'a Found> {
    all.iter().copied().filter(|one| one.id != id).collect()
}

/// 删掉目录 `dir`；本来就没有的不算错。
fn remove_dir_if_there(dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
