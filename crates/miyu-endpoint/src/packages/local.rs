//! 本地库接到装卸上（施工 F-8 中上，设计 `31-软件包.md` 第四节，`miyu_store::packages::local`）：家目录里装成、升级成的记一份
//! （每个文件的路径、SHA-256、字节数），卸掉的删掉那一份；核心起来时给以前装的、还没记的补上（照现在的文件算）。
//! `package.info`、`package.files` 照它答；出厂的不进本地库，照现在的文件现算。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_store::packages::local::{self, Desc, Entry};
use miyu_store::packages::{Found, Layer, Packages};

use super::{TARGET, packages};
use crate::Core;
use crate::refusal::Refusal;

/// 记下家目录里的包 `id`：照包目录现在的文件算，`source` 是从哪装的（补的是空的）。
///
/// # Errors
///
/// 包目录读不了、本地库写不进。
pub(super) fn record(
    places: &Packages,
    id: &str,
    version: Option<String>,
    source: &str,
) -> std::io::Result<()> {
    let home = places
        .home_dir()
        .ok_or_else(|| std::io::Error::other("no home layer"))?;
    let files = local::scan(&home.join(id))?;
    let desc = Desc {
        id: id.to_string(),
        version,
        installed: crate::sessions::now().to_string(),
        source: source.to_string(),
        size: local::size(&files),
    };
    local::write(&places.local_root(), &desc, &files)
}

/// 核心起来时：家目录里读成了、还没记的包补一份，各记一行 `INFO package recorded`；补不成的 `WARN package not recorded`。
pub(crate) fn backfill(places: &Packages, found: &[Found]) {
    let root = places.local_root();
    let known = local::ids(&root);
    for one in found {
        let Ok(manifest) = &one.read else {
            continue;
        };
        if one.layer != Layer::Home || known.contains(&one.id) {
            continue;
        }
        match record(places, &one.id, manifest.version.clone(), "") {
            Ok(()) => tracing::info!(target: TARGET, package = one.id.as_str(), "package recorded"),
            Err(error) => {
                tracing::warn!(target: TARGET, package = one.id.as_str(), error = %error, "package not recorded");
            }
        }
    }
}

/// `package.info`、`package.files` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InfoParams {
    package: String,
}

/// 一个包这时的样子：哪一层、包目录、清单写的版本，和它的记录。
pub(super) struct Looked {
    pub(super) layer: Layer,
    pub(super) dir: PathBuf,
    pub(super) version: Option<String>,
    pub(super) desc: Option<Desc>,
    pub(super) files: Vec<Entry>,
}

/// 一个包这时的记录：家目录里的照本地库（没记的现算），出厂的现算。
pub(super) async fn entry(core: &Arc<Core>, id: &str) -> Result<Looked, Refusal> {
    let now = core.packages();
    let found: &Found = now
        .iter()
        .find(|one| one.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let version = found
        .read
        .as_ref()
        .ok()
        .and_then(|manifest| manifest.version.clone());
    let places = packages(core);
    let (folder, root, layer) = (found.files_dir(), places.local_root(), found.layer);
    let target = id.to_string();
    let dir = folder.clone();
    let read = tokio::task::spawn_blocking(move || {
        let recorded = (layer == Layer::Home)
            .then(|| local::read(&root, &target))
            .flatten();
        match recorded {
            Some((desc, files)) => Ok((Some(desc), files)),
            None => local::scan(&folder).map(|files| (None, files)),
        }
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let (desc, files) = read.map_err(|error| {
        tracing::warn!(target: TARGET, package = id, error = %error, "package files unreadable");
        Refusal::INTERNAL
    })?;
    Ok(Looked {
        layer,
        dir,
        version,
        desc,
        files,
    })
}

/// `package.info {package}`：编号、哪一层、版本、装的时刻和从哪装的（本地库里记了的才有）、几个文件、多少字节。
pub(crate) async fn info(core: &Arc<Core>, params: InfoParams) -> Result<Value, Refusal> {
    let Looked {
        layer,
        version,
        desc,
        files,
        ..
    } = entry(core, &params.package).await?;
    let mut answer = json!({
        "package": params.package,
        "layer": layer.as_str(),
        "files": files.len(),
        "size": local::size(&files),
    });
    let version = desc
        .as_ref()
        .and_then(|desc| desc.version.clone())
        .or(version);
    if let Some(version) = version {
        answer["version"] = json!(version);
    }
    if let Some(desc) = desc {
        answer["installed"] = json!(desc.installed);
        if !desc.source.is_empty() {
            answer["source"] = json!(desc.source);
        }
    }
    Ok(answer)
}

/// `package.files {package}`：包目录 `dir`（绝对路径，施工 F-8 下：命令行照 pacman 印绝对路径），每个文件的 `path`（相对
/// 包目录，`/` 分开）、`sha256`、`size`，照路径排。
pub(crate) async fn files(core: &Arc<Core>, params: InfoParams) -> Result<Value, Refusal> {
    let looked = entry(core, &params.package).await?;
    let files: Vec<Value> = looked
        .files
        .iter()
        .map(|file| json!({"path": file.path, "sha256": file.sha256, "size": file.size}))
        .collect();
    Ok(json!({ "dir": looked.dir, "files": files }))
}

/// 本地库里删掉包 `id`（卸掉以后）；删不掉的记一行，不算卸失败。
pub(super) fn forget(root: &Path, id: &str) {
    if let Err(error) = local::remove(root, id) {
        tracing::warn!(target: TARGET, package = id, error = %error, "package record not removed");
    }
}
