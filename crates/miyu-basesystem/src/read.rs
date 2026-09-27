//! `read`（`10-自带软件.md` 第三节「`read` 输出的写法」，施工 4-4 上）：读文本文件，按行分页、带行号；读到
//! 目录时列出里面有什么。图片、PDF 随能接看图模型的那一步。

mod dir;
mod lines;

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use serde::Deserialize;

use miyu_fs::{Kind, OpenError, open_file, resolve};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Progress, Running, Spec, Target, Tool};

use crate::load::{self, LoadError};

/// 一次最多读几行。
pub(crate) const LINE_LIMIT: u64 = 2000;

/// `read`。
pub(crate) struct Read {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句（`software/basesystem/read/*.txt`）。
#[derive(Clone)]
pub(crate) struct Texts {
    more: Template,
    empty: Template,
    past_end: Template,
    more_entries: Template,
    missing: Template,
    not_a_file: Template,
    binary: Template,
    failed: Template,
    bad_args: Template,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    path: String,
    offset: Option<i64>,
    limit: Option<i64>,
}

impl Read {
    /// 照资源目录 `resources` 里的字造。
    pub(crate) fn load(resources: &Path) -> Result<Read, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "read", name, fields);
        Ok(Read {
            spec: load::spec(resources, "read", Access::Read)?,
            texts: Texts {
                more: text("more", &["from", "to", "total", "next"])?,
                empty: text("empty", &[])?,
                past_end: text("past-end", &["total", "offset"])?,
                more_entries: text("more-entries", &["rest"])?,
                missing: text("missing", &["path"])?,
                not_a_file: text("not-a-file", &["path"])?,
                binary: text("binary", &["path"])?,
                failed: text("failed", &["path", "error"])?,
                bad_args: text("bad-args", &["error"])?,
            },
        })
    }
}

impl Tool for Read {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn targets(&self, call: &Call) -> Vec<Target> {
        serde_json::from_str::<Args>(&call.args)
            .map(|args| {
                vec![Target {
                    path: args.path,
                    write: false,
                }]
            })
            .unwrap_or_default()
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => {
                    return Done::error(
                        texts.say(&texts.bad_args, &[("error", &error.to_string())]),
                    );
                }
            };
            // 读文件在阻塞线程里做；读的时候 panic 了，照样 panic，执行器认得出工具崩了。
            match tokio::task::spawn_blocking(move || read(&texts, &call, &args)).await {
                Ok(done) => done,
                Err(error) => std::panic::resume_unwind(error.into_panic()),
            }
        })
    }
}

impl Texts {
    /// 换进字段。
    ///
    /// # Panics
    ///
    /// 实际不会：造的时候试换过，字段都有。
    fn say(&self, template: &Template, fields: &[(&str, &str)]) -> String {
        let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
        template.render(&fields).expect("造的时候试换过，字段都有")
    }
}

/// 读：换成真实的位置，是目录就列，是文件就按行读。
fn read(texts: &Texts, call: &Call, args: &Args) -> Done {
    let path = args.path.as_str();
    let fail = |error: &dyn std::fmt::Display| {
        Done::error(texts.say(
            &texts.failed,
            &[("path", path), ("error", &error.to_string())],
        ))
    };
    let real = match resolve(Path::new(&call.cwd), call.home.as_deref(), path) {
        Ok(real) => real,
        Err(error) => return fail(&error),
    };
    let file: File = match open_file(&real) {
        Ok(file) => file,
        Err(OpenError::NotAFile(Kind::Directory)) => return dir::list(texts, path, &real),
        Err(OpenError::NotFound) => {
            return Done::error(texts.say(&texts.missing, &[("path", path)]));
        }
        Err(OpenError::NotAFile(_)) => {
            return Done::error(texts.say(&texts.not_a_file, &[("path", path)]));
        }
        Err(OpenError::Io(error)) => return fail(&error),
    };
    let offset = u64::try_from(args.offset.unwrap_or(1)).unwrap_or(1).max(1);
    let limit = u64::try_from(args.limit.unwrap_or(0))
        .ok()
        .filter(|limit| *limit > 0)
        .map_or(LINE_LIMIT, |limit| limit.min(LINE_LIMIT));
    match lines::read(file, offset, limit) {
        Ok(lines::Page::Binary) => Done::error(texts.say(&texts.binary, &[("path", path)])),
        Ok(lines::Page::Empty) => Done::ok(texts.say(&texts.empty, &[])),
        Ok(lines::Page::PastEnd { total }) => Done::ok(texts.say(
            &texts.past_end,
            &[
                ("total", &total.to_string()),
                ("offset", &offset.to_string()),
            ],
        )),
        Ok(lines::Page::Lines {
            mut text,
            from,
            to,
            total,
        }) => {
            if to < total {
                text.push_str(&texts.say(
                    &texts.more,
                    &[
                        ("from", &from.to_string()),
                        ("to", &to.to_string()),
                        ("total", &total.to_string()),
                        ("next", &(to + 1).to_string()),
                    ],
                ));
            }
            Done::ok(text)
        }
        Err(error) => fail(&error),
    }
}
