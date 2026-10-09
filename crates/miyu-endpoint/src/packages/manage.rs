//! 装、卸（施工 F-5 上，设计 `30-插件框架.md` 第九节，`docs/blueprint/packages.md`「装卸」）：`package.install`、
//! `package.remove`。只动管理员家目录那一层（磁盘上怎么做在 `miyu_store::packages::install`），做完照两层重读、当场换掉核心
//! 手里的清单（[`Core::reload_packages`]）：`package.list`、预设的功能、开着的会话下一个回合都照新的。一次只做一件。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package;
use miyu_store::packages::install::{self, Placed};
use miyu_store::packages::{Found, Issue, Layer, Packages};

use super::{TARGET, compiled, listed, load, packages, sentence, settle};
use crate::Core;
use crate::config::methods::words;
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
    /// 照两层重读软件包清单，标没编进来的内置包、认配置项撞没撞，当场换掉核心手里的那一份（施工 F-5 上）。
    pub(crate) fn reload_packages(&self) {
        let mut found = load(&self.resources, &self.root, &self.admin);
        if let Some(built_in) = &self.built_in {
            compiled(&mut found, built_in);
        }
        let items = self.config().items().to_vec();
        let _ = settle(&mut found, &items);
        self.rebuild_builtins(&found);
        *self
            .packages
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = std::sync::Arc::new(found);
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
    let _one_at_a_time = core.packaging.lock().await;
    let id = params.package;
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
    let layer = one.layer;
    let removed = blocking(move || match layer {
        Layer::Home => install::take_out(&home, &id).map(|()| id),
        Layer::Shipped => install::mark_removed(&home, &id).map(|()| id),
    })
    .await?;
    core.reload_packages();
    core.follow_packages(&found).await;
    tracing::info!(target: TARGET, package = removed.as_str(), "package removed");
    Ok(json!({"package": removed, "removed": true}))
}

/// 从 `path` 这份清单装：读得成、不和出厂的撞、拷进去以后和别的包也不撞，才算装上。
async fn from_path(core: &Arc<Core>, peer: Peer, path: &Path) -> Result<Value, Refusal> {
    let id = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".toml"))
        .filter(|id| miyu_store::personas::valid(id))
        .map(str::to_string)
        .filter(|_| path.is_absolute())
        .ok_or(Refusal::BAD_PARAMS)?;
    let text = std::fs::read_to_string(path).map_err(|_| Refusal::PATH_UNREADABLE)?;
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
    let files = path.with_extension("");
    let files = files.is_dir().then_some(files);
    let manifest = path.to_path_buf();
    let target = id.clone();
    let before = core.packages();
    let placed: Placed =
        blocking(move || install::place(&home, &target, &manifest, files.as_deref())).await?;
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
        return Err(refusal);
    }
    placed.keep();
    core.follow_packages(&before).await;
    tracing::info!(target: TARGET, package = id.as_str(), "package installed");
    let mine = now
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::INTERNAL)?;
    Ok(listed(mine, &places, &words, peer.language))
}

/// 把卸掉的出厂的包 `id` 装回来：删掉家目录里记的那一笔。
async fn bring_back(core: &Arc<Core>, peer: Peer, id: &str) -> Result<Value, Refusal> {
    let places = packages(core);
    if !places.removed().iter().any(|removed| removed == id) {
        return Err(Refusal::UNKNOWN_PACKAGE);
    }
    let home = home(core)?;
    let target = id.to_string();
    let before = core.packages();
    blocking(move || install::unmark_removed(&home, &target)).await?;
    core.reload_packages();
    core.follow_packages(&before).await;
    tracing::info!(target: TARGET, package = id, "package restored");
    let words = words(core, peer.language)?;
    let now = core.packages();
    let back = now
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    Ok(listed(back, &places, &words, peer.language))
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
