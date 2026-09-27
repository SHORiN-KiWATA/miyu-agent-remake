//! `trash`（`10-自带软件.md` 第三节「`trash` 的细则」，施工 4-6 下）：把一个文件或者目录移进系统的回收站，记下
//! 它在回收站里的位置，报 `file.trashed`，撤销（4-7）照它移回来。
//!
//! 放进回收站三个平台各做一份，在 [`miyu_fs::trash`] 里（施工 4-7 上从这里挪过去，撤销也要用）。记下的位置三个平台
//! 都是回收站里的真实路径。回收站收不了的不删，说为什么。工作目录本身和它的上级、家目录、根目录不许删；链接删的是
//! 链接本身。

use std::path::{Path, PathBuf};

use serde::Deserialize;

use miyu_fs::trash::{Refused, put};
use miyu_fs::{ResolveError, resolve, tilde};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect, Progress, Running, Spec, Target, Tool};

use crate::blocking::blocking;
use crate::common::{Common, Shown, said};
use crate::load::{self, LoadError, say};

/// `trash`。
pub(crate) struct Trash {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/trash/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    trashed: Template,
    unavailable: Template,
    protected: Template,
    lost: Template,
    failed: Template,
}

/// 她给的参数：写成 `path` 的也认。
#[derive(Deserialize)]
struct Args {
    #[serde(alias = "path", alias = "filePath")]
    file_path: String,
}

impl Trash {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Trash, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "trash", name, fields);
        Ok(Trash {
            spec: load::spec(resources, "trash", Access::Write)?,
            texts: Texts {
                common,
                trashed: text("trashed", &["path"])?,
                unavailable: text("unavailable", &["path"])?,
                protected: text("protected", &["path"])?,
                lost: text("lost", &["path"])?,
                failed: text("failed", &["path", "error"])?,
            },
        })
    }
}

impl Tool for Trash {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn targets(&self, call: &Call) -> Vec<Target> {
        serde_json::from_str::<Args>(&call.args)
            .map(|args| {
                vec![Target {
                    path: args.file_path,
                    write: true,
                }]
            })
            .unwrap_or_default()
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => blocking(move |_| trash(&texts, &call, &args.file_path)).await,
                Err(error) => texts.common.bad_args(&error),
            }
        })
    }
}

/// 删：换成真实的位置（最后一段不跟链接），不许删的不删，照平台移进回收站。
fn trash(texts: &Texts, call: &Call, path: &str) -> Done {
    let refuse = |template: &Template, key: &str| {
        Done::error(say(template, &[("path", path)])).said(said(key))
    };
    let real = match located(call, path) {
        Ok(Some(real)) => real,
        Ok(None) => return refuse(&texts.protected, "trash/protected"),
        Err(error) => return texts.common.failed(path, &error),
    };
    if std::fs::symlink_metadata(&real).is_err() {
        return texts.common.missing(path, &real, &Shown::here(call));
    }
    if protected(call, &real) {
        return refuse(&texts.protected, "trash/protected");
    }
    match put(&real, call.home.as_deref()) {
        Ok(location) => {
            let shown = Shown::here(call).path(&real);
            Done::ok(say(&texts.trashed, &[("path", &shown)]))
                .said(said("trash/trashed"))
                .effect(Effect::Trashed {
                    path: real,
                    trash: location,
                })
        }
        Err(Refused::Unavailable) => refuse(&texts.unavailable, "trash/unavailable"),
        Err(Refused::Lost) => refuse(&texts.lost, "trash/lost"),
        Err(Refused::Failed(error)) => {
            let error = error.to_string();
            Done::error(say(&texts.failed, &[("path", path), ("error", &error)]))
                .said(said("trash/failed").with("error", error))
        }
    }
}

/// 她给的 `path` 换成真实的位置，最后一段不跟链接：上级目录换成真的，再接上名字，删的是这个名字。`~` 开头的照
/// [`tilde`] 接家目录，和别的工具一个规矩。没有名字可删的（`.`、`..`、根目录、`~` 本身）是空的：不许删。
fn located(call: &Call, path: &str) -> Result<Option<PathBuf>, ResolveError> {
    let expanded = match tilde(path) {
        Some("") => return Ok(None),
        Some(rest) => call.home.as_deref().ok_or(ResolveError::NoHome)?.join(rest),
        None => PathBuf::from(path),
    };
    let Some(name) = expanded.file_name() else {
        return Ok(None);
    };
    let parent = expanded.parent().unwrap_or(Path::new(""));
    let real = resolve(
        Path::new(&call.cwd),
        call.home.as_deref(),
        &parent.to_string_lossy(),
    )?;
    Ok(Some(real.join(name)))
}

/// 不许删的：工作目录本身和它的每一层上级（根目录也是它的上级）、系统的家目录。
fn protected(call: &Call, real: &Path) -> bool {
    if let Ok(cwd) = resolve(Path::new(&call.cwd), call.home.as_deref(), &call.cwd)
        && cwd.starts_with(real)
    {
        return true;
    }
    call.home.as_deref().is_some_and(|home| {
        let home = std::fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
        home == real
    })
}
