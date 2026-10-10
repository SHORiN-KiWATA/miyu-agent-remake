//! 查装好的文件（施工 F-8 中下，设计 `31-软件包.md` 第四节，照 pacman 的 `-Qo`、`-Qk`）：
//!
//! - `package.owns {path}`：这个路径在哪个包的目录里（两层都找，换成真实的位置比），本地库里有没有记它。
//! - `package.check {package?}`：家目录里本地库记了的包，照记的一个个比哈希：改了的、少了的、多出来的（多出来的只是提示：
//!   包自己运行时写的也算）。出厂的不进本地库，不查。
//!
//! `miyu check` 也报（端点的 `check.rs`）：改了、少了的各一条警告。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_store::packages::local;
use miyu_store::packages::{Found, Layer};

use super::{TARGET, packages};
use crate::Core;
use crate::refusal::Refusal;

/// `package.owns` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OwnsParams {
    path: String,
}

/// `package.check` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckParams {
    #[serde(default)]
    package: Option<String>,
}

/// 一个包比出来的：改了的、少了的、多出来的，都是相对包目录的路径，照路径排。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Drift {
    pub(crate) modified: Vec<String>,
    pub(crate) missing: Vec<String>,
    pub(crate) extra: Vec<String>,
}

/// 真实的位置；换不成的照原样。
fn real(path: &Path) -> PathBuf {
    miyu_fs::resolve(Path::new("/"), None, &path.to_string_lossy())
        .unwrap_or_else(|_| path.to_path_buf())
}

/// `package.owns {path}`：`path` 要是绝对路径（命令行照当前目录接好）。回应 `{"package": 编号 | null}`，有的多 `layer`、
/// `path`（相对包目录）、`recorded`（本地库里有没有记它；出厂的总是假）。
pub(crate) async fn owns(core: &Arc<Core>, params: OwnsParams) -> Result<Value, Refusal> {
    let wanted = PathBuf::from(&params.path);
    if !wanted.is_absolute() {
        return Err(Refusal::BAD_PARAMS);
    }
    let now = core.packages();
    let folders: Vec<(String, Layer, PathBuf)> = now
        .iter()
        .map(|one: &Found| (one.id.clone(), one.layer, one.files_dir()))
        .collect();
    let root = packages(core).local_root();
    let answer = tokio::task::spawn_blocking(move || {
        let target = real(&wanted);
        for (id, layer, folder) in folders {
            let folder = real(&folder);
            let Ok(rest) = target.strip_prefix(&folder) else {
                continue;
            };
            let relative = rest
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let recorded = layer == Layer::Home
                && local::read(&root, &id)
                    .is_some_and(|(_, files)| files.iter().any(|file| file.path == relative));
            return json!({"package": id, "layer": layer.as_str(), "path": relative, "recorded": recorded});
        }
        json!({"package": null})
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    Ok(answer)
}

/// 照本地库比包 `id` 的包目录 `folder`；本地库里没记的没有。
pub(crate) fn drift(root: &Path, id: &str, folder: &Path) -> Option<Drift> {
    let (_, recorded) = local::read(root, id)?;
    let now = local::scan(folder).unwrap_or_default();
    let mut drift = Drift::default();
    for file in &recorded {
        match now.iter().find(|one| one.path == file.path) {
            None => drift.missing.push(file.path.clone()),
            Some(one) if one.sha256 != file.sha256 => drift.modified.push(file.path.clone()),
            Some(_) => {}
        }
    }
    drift.extra = now
        .iter()
        .filter(|one| !recorded.iter().any(|file| file.path == one.path))
        .map(|one| one.path.clone())
        .collect();
    Some(drift)
}

/// 家目录里本地库记了的包，照编号排：编号、包目录。只要 `only` 的那一个（写了的话）。
pub(crate) fn recorded(core: &Core, only: Option<&str>) -> Vec<(String, PathBuf)> {
    let now = core.packages();
    let mut all: Vec<(String, PathBuf)> = now
        .iter()
        .filter(|one| one.layer == Layer::Home)
        .filter(|one| only.is_none_or(|id| one.id == id))
        .map(|one| (one.id.clone(), one.files_dir()))
        .collect();
    all.sort();
    all
}

/// `package.check {package?}`：`{"packages": [{"package", "modified", "missing", "extra"}]}`，本地库里没记的不在里面；写了
/// `package` 的只查它，没装的 `unknown_package`。
pub(crate) async fn check(core: &Arc<Core>, params: CheckParams) -> Result<Value, Refusal> {
    if let Some(id) = &params.package
        && !core.packages().iter().any(|one| &one.id == id)
    {
        return Err(Refusal::UNKNOWN_PACKAGE);
    }
    let wanted = recorded(core, params.package.as_deref());
    let root = packages(core).local_root();
    let checked = tokio::task::spawn_blocking(move || {
        wanted
            .into_iter()
            .filter_map(|(id, folder)| drift(&root, &id, &folder).map(|drift| (id, drift)))
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let packages: Vec<Value> = checked
        .into_iter()
        .map(|(id, drift)| {
            if !drift.modified.is_empty() || !drift.missing.is_empty() {
                tracing::info!(target: TARGET, package = id.as_str(), modified = drift.modified.len(), missing = drift.missing.len(), "package files changed");
            }
            json!({"package": id, "modified": drift.modified, "missing": drift.missing, "extra": drift.extra})
        })
        .collect();
    Ok(json!({ "packages": packages }))
}

/// 家目录里本地库记了的、文件改了或少了的包：清单在哪、比出来的（`miyu check` 用）。
pub(crate) async fn drifted(core: &Core) -> Vec<(PathBuf, Drift)> {
    let wanted: Vec<(String, PathBuf)> = recorded(core, None);
    let root = packages(core).local_root();
    tokio::task::spawn_blocking(move || {
        wanted
            .into_iter()
            .filter_map(|(id, folder)| {
                let drift = drift(&root, &id, &folder)?;
                (!drift.modified.is_empty() || !drift.missing.is_empty())
                    .then(|| (folder.join(miyu_store::packages::MANIFEST), drift))
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}
