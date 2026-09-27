//! `glob`（`10-自带软件.md` 第三节、第十节，施工 4-4 下）：按名字模式找文件，遵守 `.gitignore`，按修改时间新的
//! 在前，最多 [`LIMIT`] 个。模式的规矩在 [`crate::pattern`]，走目录的规矩在 [`crate::walk`]。

use std::path::Path;

use serde::Deserialize;

use miyu_fs::resolve;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Progress, Running, Spec, Target, Tool};

use crate::blocking::{Stop, blocking};
use crate::common::{Common, Shown, given, said};
use crate::load::{self, LoadError, say};
use crate::pattern::{Pattern, split_absolute};
use crate::walk;

/// 最多列几个：Claude Code、opencode 都是 100，都不给改。
pub(crate) const LIMIT: usize = 100;

/// `glob`。
pub(crate) struct Glob {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/glob/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    more: Template,
    not_a_directory: Template,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    pattern: String,
    path: Option<String>,
}

impl Args {
    /// 搜的目录（她给的写法）和相对它的模式。模式本身是绝对路径的，拆出前面那一截当目录；不然目录是 `path`，
    /// 没给就是工作目录。
    fn place(&self) -> (String, String) {
        match split_absolute(&self.pattern) {
            Some((dir, rest)) => (dir.to_string_lossy().into_owned(), rest),
            None => (
                given(self.path.clone()).unwrap_or_else(|| ".".to_string()),
                self.pattern.clone(),
            ),
        }
    }
}

impl Glob {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Glob, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "glob", name, fields);
        Ok(Glob {
            spec: load::spec(resources, "glob", Access::Read)?,
            texts: Texts {
                common,
                more: text("more", &["shown", "total", "rest"])?,
                not_a_directory: text("not-a-directory", &["path"])?,
            },
        })
    }
}

impl Tool for Glob {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn targets(&self, call: &Call) -> Vec<Target> {
        serde_json::from_str::<Args>(&call.args)
            .map(|args| {
                vec![Target {
                    path: args.place().0,
                    write: false,
                }]
            })
            .unwrap_or_default()
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => blocking(move |stop| find(&texts, &call, &args, stop)).await,
                Err(error) => texts.common.bad_args(&error),
            }
        })
    }
}

/// 找：搜的目录换成真实的位置，走一遍，照模式留下对得上的。
fn find(texts: &Texts, call: &Call, args: &Args, stop: &Stop) -> Done {
    let (dir, pattern) = args.place();
    let pattern = match Pattern::new(&pattern) {
        Ok(pattern) => pattern,
        Err(error) => return texts.common.bad_glob(&args.pattern, &error),
    };
    let shown = Shown::here(call);
    let real = match resolve(Path::new(&call.cwd), call.home.as_deref(), &dir) {
        Ok(real) => real,
        Err(error) => return texts.common.failed(&dir, &error),
    };
    match std::fs::metadata(&real) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            return Done::error(say(&texts.not_a_directory, &[("path", &dir)]))
                .said(said("glob/not-a-directory").with("path", dir.as_str()));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return texts.common.missing(&dir, &real, &shown);
        }
        Err(error) => return texts.common.failed(&dir, &error),
    }
    let found = walk::files(&real, walk::Fence::of(call), stop, |relative| {
        pattern.matches(relative)
    });
    if found.is_empty() {
        return texts.common.no_files();
    }
    let mut text = String::new();
    for file in found.iter().take(LIMIT) {
        text.push_str(&shown.path(&file.path));
        text.push('\n');
    }
    let total = found.len().to_string();
    if let Some(rest) = found.len().checked_sub(LIMIT).filter(|rest| *rest > 0) {
        text.push_str(&say(
            &texts.more,
            &[
                ("shown", &LIMIT.to_string()),
                ("total", &total),
                ("rest", &rest.to_string()),
            ],
        ));
        return Done::ok(text).said(
            said("glob/files-more")
                .with("shown", LIMIT.to_string())
                .with("total", total),
        );
    }
    Done::ok(text).said(said("glob/files").with("count", total))
}
