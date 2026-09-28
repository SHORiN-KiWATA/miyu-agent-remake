//! `read`（`10-自带软件.md` 第三节「`read` 输出的写法」，施工 4-4 上；施工 4-4 下照第十节改到规范上）：读文本
//! 文件，按行分页、带行号；读到图片交回图片本身（施工 4-13）；读到目录时列出里面有什么，一样分页。PDF 另算。

mod dir;
mod image;
pub(crate) mod lines;

use std::fs::File;
use std::io::{Read as _, Seek};
use std::path::Path;

use serde::Deserialize;

use miyu_fs::{Kind, OpenError, open_file, resolve};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect, Progress, Running, Spec, Target, Tool};

use crate::blocking::blocking;
use crate::common::{Common, Shown, said};
use crate::load::{self, LoadError, say};

/// 一次最多读几行，目录一次最多列几项。
pub(crate) const LINE_LIMIT: u64 = 2000;

/// `read`。
pub(crate) struct Read {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/read/*.txt`，和几件工具共用的。
#[derive(Clone)]
pub(crate) struct Texts {
    common: Common,
    more: Template,
    empty: Template,
    past_end: Template,
    more_entries: Template,
    past_end_entries: Template,
    not_a_file: Template,
    binary: Template,
    image_too_big: Template,
    image_too_wide: Template,
}

/// 她给的参数。名字照 Claude Code 叫 `file_path`；照 pi 写成 `path`、照 opencode 写成 `filePath` 的也认。
#[derive(Deserialize)]
struct Args {
    #[serde(alias = "path", alias = "filePath")]
    file_path: String,
    offset: Option<i64>,
    limit: Option<i64>,
}

impl Args {
    /// 从第几行（第几项）起，从 1 数起：没给、给了 0 或者负数，都从头。
    fn offset(&self) -> u64 {
        u64::try_from(self.offset.unwrap_or(1)).unwrap_or(1).max(1)
    }

    /// 最多几行（几项）：没给、给了 0 或者负数，照 [`LINE_LIMIT`]；多过它的也照它。
    fn limit(&self) -> u64 {
        u64::try_from(self.limit.unwrap_or(0))
            .ok()
            .filter(|limit| *limit > 0)
            .map_or(LINE_LIMIT, |limit| limit.min(LINE_LIMIT))
    }
}

impl Read {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Read, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "read", name, fields);
        Ok(Read {
            spec: load::spec(resources, "read", Access::Read)?,
            texts: Texts {
                common,
                more: text("more", &["from", "to", "total", "next"])?,
                empty: text("empty", &[])?,
                past_end: text("past-end", &["total", "offset"])?,
                more_entries: text("more-entries", &["from", "to", "total", "next"])?,
                past_end_entries: text("past-end-entries", &["total", "offset"])?,
                not_a_file: text("not-a-file", &["path"])?,
                binary: text("binary", &["path"])?,
                image_too_big: text("image-too-big", &["path", "size"])?,
                image_too_wide: text("image-too-wide", &["path", "width", "height"])?,
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
                    path: args.file_path,
                    write: false,
                    itself: false,
                }]
            })
            .unwrap_or_default()
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => blocking(call.stop.clone(), move |_| read(&texts, &call, &args)).await,
                Err(error) => texts.common.bad_args(&error),
            }
        })
    }
}

/// 读：换成真实的位置，是目录就列，是文件就按行读。
fn read(texts: &Texts, call: &Call, args: &Args) -> Done {
    let path = args.file_path.as_str();
    let real = match resolve(Path::new(&call.cwd), call.home.as_deref(), path) {
        Ok(real) => real,
        Err(error) => return texts.common.failed(path, &error),
    };
    let (offset, limit) = (args.offset(), args.limit());
    let mut file: File = match open_file(&real) {
        Ok(file) => file,
        Err(OpenError::NotAFile(Kind::Directory)) => {
            return dir::list(texts, path, &real, offset, limit);
        }
        Err(OpenError::NotFound) => {
            return texts.common.missing(path, &real, &Shown::here(call));
        }
        Err(OpenError::NotAFile(_)) => {
            return Done::error(say(&texts.not_a_file, &[("path", path)]))
                .said(said("read/not-a-file").with("path", path));
        }
        Err(OpenError::Io(error)) => return texts.common.failed(path, &error),
    };
    // 先看开头认图片（施工 4-13）；不是的倒回开头，照旧认编码、按行读。
    let mut head = Vec::with_capacity(image::HEAD);
    if let Err(error) = (&mut file).take(image::HEAD as u64).read_to_end(&mut head) {
        return texts.common.failed(path, &error);
    }
    if let Some(media_type) = image::kind(&head) {
        return image::read(texts, path, real, file, head, media_type);
    }
    if let Err(error) = file.rewind() {
        return texts.common.failed(path, &error);
    }
    let paged = match lines::read(file, offset, limit) {
        Ok(paged) => paged,
        Err(error) => return texts.common.failed(path, &error),
    };
    let (done, shown) = match paged.page {
        // 二进制的不给内容，照样报读过（施工 4-9 再补二）：她知道它在，`write` 盖它之前照它核对。
        lines::Page::Binary => (
            Done::error(say(&texts.binary, &[("path", path)]))
                .said(said("read/binary").with("path", path)),
            None,
        ),
        lines::Page::Empty => (
            Done::ok(say(&texts.empty, &[])).said(said("read/empty")),
            None,
        ),
        lines::Page::PastEnd { total } => (
            Done::ok(say(
                &texts.past_end,
                &[
                    ("total", &total.to_string()),
                    ("offset", &offset.to_string()),
                ],
            ))
            .said(
                said("read/past-end")
                    .with("total", total.to_string())
                    .with("offset", offset.to_string()),
            ),
            None,
        ),
        lines::Page::Lines {
            mut text,
            from,
            to,
            total,
        } => {
            if to < total {
                text.push_str(&say(
                    &texts.more,
                    &[
                        ("from", &from.to_string()),
                        ("to", &to.to_string()),
                        ("total", &total.to_string()),
                        ("next", &(to + 1).to_string()),
                    ],
                ));
            }
            (
                Done::ok(text).said(part("read/lines", from, to, total)),
                Some([from, to]),
            )
        }
    };
    // 读到了一个文件：报 `file.read`，她改之前照它核对（施工 4-6 上）。二进制的也报，没有行的范围。
    match paged.hash {
        Some(hash) => done.effect(Effect::Read {
            path: real,
            lines: shown,
            hash,
        }),
        None => done,
    }
}

/// 读了第 `from` 到第 `to`（一共 `total`）行或者项，给人看的说法：读全了的是 `<key>`，只读了一段的是
/// `<key>-part`。
pub(crate) fn part(key: &str, from: u64, to: u64, total: u64) -> miyu_kernel::event::Said {
    if from == 1 && to == total {
        said(key).with("count", total.to_string())
    } else {
        said(&format!("{key}-part"))
            .with("from", from.to_string())
            .with("to", to.to_string())
            .with("total", total.to_string())
    }
}
