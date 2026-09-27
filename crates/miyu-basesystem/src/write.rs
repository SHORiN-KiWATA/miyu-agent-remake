//! `write`（`10-自带软件.md` 第三节「`write` 的细则」，施工 4-6 上）：新建一个文件，或者整体覆盖一个文件。参数照
//! Claude Code 叫 `file_path`、`content`。
//!
//! 已经在了的文件，要她看过、而且现在的内容和她看到的一样才写（第五节「她看过的」）；照它原来的编码、BOM、换行
//! 写。新文件写成 UTF-8、不带 BOM，上级目录没有的建上。写的时候先写临时文件再改名盖上去（[`miyu_fs::replace()`]）。
//! 报 `file.changed`：改前改后的内容本身，执行器存成 blob。

use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use miyu_fs::{replace, resolve};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect, Progress, Running, Spec, Target, Tool};

use crate::blocking::blocking;
use crate::common::{Common, Shown, said};
use crate::load::{self, LoadError, say};
use crate::text::{Style, line_count};

/// `write`。
pub(crate) struct Write {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/write/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    created: Template,
    updated: Template,
}

/// 她给的参数。照 pi 写成 `path`、照 opencode 写成 `filePath` 的也认，和 `read` 一样。
#[derive(Deserialize)]
struct Args {
    #[serde(alias = "path", alias = "filePath")]
    file_path: String,
    content: String,
}

impl Write {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Write, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "write", name, fields);
        Ok(Write {
            spec: load::spec(resources, "write", Access::Write)?,
            texts: Texts {
                common,
                created: text("created", &["path"])?,
                updated: text("updated", &["path"])?,
            },
        })
    }
}

impl Tool for Write {
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
                Ok(args) => blocking(move |_| write(&texts, &call, &args)).await,
                Err(error) => texts.common.bad_args(&error),
            }
        })
    }
}

/// 写：换成真实的位置，看它现在是什么；已经在了的先核对她看过的，再照原来的写法写。
fn write(texts: &Texts, call: &Call, args: &Args) -> Done {
    let path = args.file_path.as_str();
    let failed = |error: &dyn std::fmt::Display| texts.common.write_failed(path, error);
    let real = match resolve(Path::new(&call.cwd), call.home.as_deref(), path) {
        Ok(real) => real,
        Err(error) => return failed(&error),
    };
    let before = match fs::symlink_metadata(&real) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return failed(&error),
        Ok(meta) if meta.is_dir() => return texts.common.directory(path),
        Ok(meta) if !meta.is_file() => return texts.common.not_regular(path),
        Ok(_) => match fs::read(&real) {
            Ok(bytes) => Some(bytes),
            Err(error) => return failed(&error),
        },
    };
    let after = match &before {
        Some(old) => {
            // 改之前核对：她没看过的不写；看过、可现在的内容和她看到的不一样的，也不写。
            if let Some(refused) = texts.common.unseen(call, path, &real, old) {
                return refused;
            }
            Style::of(old).encode(&args.content)
        }
        None => {
            if let Some(dir) = real.parent()
                && let Err(error) = fs::create_dir_all(dir)
            {
                return failed(&error);
            }
            Style::fresh().encode(&args.content)
        }
    };
    if let Err(error) = replace(&real, &after) {
        return failed(&error);
    }
    let (template, key) = match before {
        Some(_) => (&texts.updated, "write/updated"),
        None => (&texts.created, "write/created"),
    };
    let shown = Shown::here(call).path(&real);
    Done::ok(say(template, &[("path", &shown)]))
        .said(said(key).with("count", line_count(&args.content).to_string()))
        .effect(Effect::Changed {
            path: real,
            before,
            after,
        })
}
