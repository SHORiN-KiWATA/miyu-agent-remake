//! `fs.list`、`fs.find`（施工 W-2，`docs/blueprint/web-module.md`「三、列文件、找文件」）：列一层目录、在一个
//! 目录里模糊找文件。两个方法的回应一个样子：`items` 每一条 `{dir, full, marks, path, size}`；数据根只有账号
//! 自己的工作区能列、能找，落进去的 `path_forbidden`，换不成真实的位置、不是目录的 `path_unreadable`。
//!
//! 列目录、建清单、打分住在 `miyu-fs`（[`miyu_fs::list_dir`]、[`miyu_fs::Index`]、[`miyu_fs::score`]）；这里只管
//! 参数、边界表、回应的 JSON，和找文件的清单记几份（[`Cache`]）。

mod cache;

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_fs::{Boundary, Places, Zone, resolve};
use miyu_kernel::id::AccountId;
use miyu_store::root::DataRoot;

use crate::Core;
use crate::refusal::Refusal;
pub(crate) use cache::Cache;

/// 运行日志的来源。
const TARGET: &str = "miyu::endpoint";

/// `fs.list` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct ListParams {
    /// 相对的路径照它接：绝对路径，或者 `~`、`~/…`。
    cwd: String,
    /// 打的那一截目录：`~` 打头的照家目录，绝对的照原样，别的照 `cwd`。
    #[serde(default)]
    dir: String,
    /// 名字的开头。
    #[serde(default)]
    prefix: String,
}

/// `fs.find` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct FindParams {
    /// 在哪个目录里找，写法同 `fs.list` 的 `cwd`。
    cwd: String,
    /// 打的字。
    #[serde(default)]
    query: String,
    /// 头开列表时写 `true`：清单建好 10 秒以上的重建。
    #[serde(default)]
    fresh: bool,
}

/// 读、存要的几样：数据根、管理员、系统的家目录。
struct Place {
    root: DataRoot,
    admin: AccountId,
    home: Option<PathBuf>,
}

/// `fs.list`：列 `cwd`/`dir` 这一层。
pub(crate) async fn list(core: &Core, params: ListParams) -> Result<Value, Refusal> {
    let place = place(core);
    blocking(move || list_blocking(&place, params)).await
}

fn list_blocking(place: &Place, params: ListParams) -> Result<Value, Refusal> {
    let boundary = boundary_of(place);
    let cwd_real = require_dir(
        Path::new("/"),
        place.home.as_deref(),
        &params.cwd,
        &boundary,
    )?;
    let real = if params.dir.is_empty() {
        cwd_real
    } else {
        require_dir(&cwd_real, place.home.as_deref(), &params.dir, &boundary)?
    };
    let (entries, partial) = miyu_fs::list_dir(&real, &params.prefix, &boundary)
        .map_err(|_| Refusal::PATH_UNREADABLE)?;
    let marks: Vec<usize> = (0..params.prefix.chars().count()).collect();
    let items: Vec<Value> = entries
        .iter()
        .map(|entry| item_json(entry.dir, &entry.full, &entry.path, &marks))
        .collect();
    Ok(json!({"items": items, "partial": partial}))
}

/// `fs.find`：在 `cwd` 里模糊找，清单记在核心的 [`Cache`] 里。
pub(crate) async fn find(core: &Core, params: FindParams) -> Result<Value, Refusal> {
    let place = place(core);
    let FindParams { cwd, query, fresh } = params;
    let (real, boundary) = blocking(move || {
        let boundary = boundary_of(&place);
        let real = require_dir(Path::new("/"), place.home.as_deref(), &cwd, &boundary)?;
        Ok((real, boundary))
    })
    .await?;
    let index = core.files.get(&real, &boundary, fresh, core.files_fresh);
    let scored = index.with(|built| snapshot(built, &query));
    let items: Vec<Value> = scored
        .hits
        .into_iter()
        .map(|hit| {
            let full = real.join(hit.path.trim_end_matches('/'));
            item_json(hit.dir, &full, &hit.path, &hit.marks)
        })
        .collect();
    Ok(json!({"building": !scored.done, "items": items, "partial": scored.partial}))
}

/// 打分排过序的一条：路径、是不是目录、对上的字（第几个字）。
struct Hit {
    path: String,
    dir: bool,
    marks: Vec<usize>,
}

/// 照 `query` 打分的结果：排过序、截到 [`miyu_fs::SHOWN`] 条的 `hits`，清单建完了没有，列全了没有。
struct Scored {
    hits: Vec<Hit>,
    done: bool,
    partial: bool,
}

/// 照 `query` 打分、排序、截到 [`miyu_fs::SHOWN`] 条：分高的在前，一样的路径短的在前，再一样的照字排
/// （`web-module.md`「怎么走」第 7 条）。`partial`：清单本来就没收全，或者对上的比截出来的还多。
fn snapshot(built: &miyu_fs::Built, query: &str) -> Scored {
    let mut hits: Vec<(i64, usize, &miyu_fs::Found, Vec<usize>)> = built
        .entries
        .iter()
        .filter_map(|found| {
            miyu_fs::score(&found.path, query)
                .map(|(score, marks)| (score, found.path.chars().count(), found, marks))
        })
        .collect();
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.path.cmp(&b.2.path))
    });
    let matched = hits.len();
    hits.truncate(miyu_fs::SHOWN);
    let hits = hits
        .into_iter()
        .map(|(_, _, found, marks)| Hit {
            path: found.path.clone(),
            dir: found.dir,
            marks,
        })
        .collect();
    Scored {
        hits,
        done: built.done,
        partial: built.partial || matched > miyu_fs::SHOWN,
    }
}

/// 一条列目录、找文件的结果，照协议的回应形状（`web-module.md`「每个方法的参数和回应」）。
fn item_json(dir: bool, full: &Path, path: &str, marks: &[usize]) -> Value {
    let mut item = Map::new();
    item.insert("dir".to_string(), json!(dir));
    item.insert("full".to_string(), json!(full.display().to_string()));
    item.insert("marks".to_string(), json!(marks));
    item.insert("path".to_string(), json!(path));
    if !dir && let Ok(meta) = std::fs::metadata(full) {
        item.insert("size".to_string(), json!(meta.len()));
    }
    Value::Object(item)
}

/// 边界表：这个账号的工作区、数据根、这台机器的临时目录和系统目录（`fs.md` 第一节）。
fn boundary_of(place: &Place) -> Boundary {
    let places = Places::here(
        place.root.workspace(&place.admin),
        place.root.path().to_path_buf(),
        place.home.as_deref(),
    );
    Boundary::new(&places)
}

/// `input` 接在 `base` 后面换成真实的位置，要是一个目录、不落进「谁都不能碰」那一片。
fn require_dir(
    base: &Path,
    home: Option<&Path>,
    input: &str,
    boundary: &Boundary,
) -> Result<PathBuf, Refusal> {
    let real = resolve(base, home, input).map_err(|_| Refusal::PATH_UNREADABLE)?;
    let is_dir = std::fs::metadata(&real)
        .map(|meta| meta.is_dir())
        .unwrap_or(false);
    if !is_dir {
        return Err(Refusal::PATH_UNREADABLE);
    }
    if boundary.zone(&real) == Zone::Forbidden {
        return Err(Refusal::PATH_FORBIDDEN);
    }
    Ok(real)
}

fn place(core: &Core) -> Place {
    Place {
        root: core.root.clone(),
        admin: core.admin.clone(),
        home: core.home.clone(),
    }
}

/// 在阻塞线程里跑；崩了的是核心的问题。
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Refusal> + Send + 'static,
) -> Result<T, Refusal> {
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(target: TARGET, error = %error, "files panicked");
            Err(Refusal::INTERNAL)
        })
}
