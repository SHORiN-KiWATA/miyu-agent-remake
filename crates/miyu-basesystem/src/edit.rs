//! `edit`（`10-自带软件.md` 第六节 B6、「`edit` 的细则」，施工 4-6 中）：在已有的文件里做精确替换，一次可以改几处。
//! 字段名照 Claude Code：`file_path`，`edits` 里每一项 `old_string`、`new_string`、`replace_all`。
//!
//! 每一处都对照原文件找（[`find`]），唯一、不重叠；有一处出错，一处都不改。改之前和 `write` 一样核对她看过的；
//! 对上的那一段换成 `new_string`（换行照文件的），别的地方一个字节不动，编码、BOM 照原来的。报 `file.changed`。

mod find;

use std::ops::Range;
use std::path::Path;

use serde::Deserialize;

use miyu_fs::{Kind, OpenError, replace, resolve};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect, Progress, Running, Spec, Stop, Target, Tool};

use crate::blocking::blocking;
use crate::common::{Common, Shown, contents, said};
use crate::load::{self, LoadError, say};
use crate::text::Style;
use find::Found;

/// `edit`。
pub(crate) struct Edit {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/edit/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    edited: Template,
    no_edits: Template,
    empty: Template,
    same: Template,
    not_found: Template,
    closest: Template,
    not_unique: Template,
    overlap: Template,
    not_text: Template,
}

/// 她给的参数。`file_path` 写成 `path`、`filePath` 的也认；照 Claude Code 写成一处的（顶层的 `old_string`、
/// `new_string`、`replace_all`），当只有一项；opencode、pi 的字段名也认。
#[derive(Deserialize)]
struct Args {
    #[serde(alias = "path", alias = "filePath")]
    file_path: String,
    #[serde(default)]
    edits: Option<Edits>,
    #[serde(default, alias = "oldString", alias = "oldText")]
    old_string: Option<String>,
    #[serde(default, alias = "newString", alias = "newText")]
    new_string: Option<String>,
    #[serde(default, alias = "replaceAll")]
    replace_all: bool,
}

/// `edits`：一个列表，写成一个对象的也认。
#[derive(Deserialize)]
#[serde(untagged)]
enum Edits {
    Many(Vec<Change>),
    One(Change),
}

/// 要改的一处。
#[derive(Deserialize)]
struct Change {
    #[serde(alias = "oldString", alias = "oldText")]
    old_string: String,
    #[serde(alias = "newString", alias = "newText")]
    new_string: String,
    #[serde(default, alias = "replaceAll")]
    replace_all: bool,
}

impl Args {
    /// 要改的几处。都没给的是空的。
    fn changes(self) -> Vec<Change> {
        match self.edits {
            Some(Edits::Many(changes)) => changes,
            Some(Edits::One(change)) => vec![change],
            None => match (self.old_string, self.new_string) {
                (Some(old_string), Some(new_string)) => vec![Change {
                    old_string,
                    new_string,
                    replace_all: self.replace_all,
                }],
                _ => Vec::new(),
            },
        }
    }
}

impl Edit {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Edit, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "edit", name, fields);
        Ok(Edit {
            spec: load::spec(resources, "edit", Access::Write)?,
            texts: Texts {
                common,
                edited: text("edited", &["path"])?,
                no_edits: text("no-edits", &[])?,
                empty: text("empty", &["index"])?,
                same: text("same", &["index"])?,
                not_found: text("not-found", &["index", "path"])?,
                closest: text("closest", &["from", "to"])?,
                not_unique: text("not-unique", &["index", "path", "count", "lines"])?,
                overlap: text("overlap", &["first", "second", "path"])?,
                not_text: text("not-text", &["path"])?,
            },
        })
    }
}

impl Tool for Edit {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn targets(&self, call: &Call) -> Vec<Target> {
        serde_json::from_str::<Args>(&call.args)
            .map(|args| {
                vec![Target {
                    path: args.file_path,
                    write: true,
                    itself: false,
                }]
            })
            .unwrap_or_default()
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => {
                    let path = args.file_path.clone();
                    let changes = args.changes();
                    blocking(call.stop.clone(), move |stop| {
                        edit(&texts, &call, &path, &changes, stop)
                    })
                    .await
                }
                Err(error) => texts.common.bad_args(&error),
            }
        })
    }
}

/// 对上的一段：原文里的位置，是第几处（从 1 数起），换成什么（换行已经照文件的）。
struct Place {
    range: Range<usize>,
    index: usize,
    new: String,
}

/// 改：换成真实的位置，读原文，核对她看过的，找每一处，查重叠，从后往前换，写回去。写回去之前看一眼旗（施工
/// 4-9 再补一）：叫停了就不改，交回 `stopped`。
fn edit(texts: &Texts, call: &Call, path: &str, changes: &[Change], stop: &Stop) -> Done {
    if changes.is_empty() {
        return Done::error(say(&texts.no_edits, &[])).said(said("edit/no-edits"));
    }
    let real = match resolve(Path::new(&call.cwd), call.home.as_deref(), path) {
        Ok(real) => real,
        Err(error) => return texts.common.failed(path, &error),
    };
    let shown = Shown::here(call);
    let old = match contents(&real) {
        Ok(Some(bytes)) => bytes,
        Ok(None) | Err(OpenError::NotFound) => return texts.common.missing(path, &real, &shown),
        Err(OpenError::NotAFile(Kind::Directory)) => return texts.common.directory(path),
        Err(OpenError::NotAFile(_)) => return texts.common.not_regular(path),
        Err(OpenError::Io(error)) => return texts.common.failed(path, &error),
    };
    if let Some(refused) = texts.common.unseen(call, path, &real, &old) {
        return refused;
    }
    let style = Style::of(&old);
    let Some(text) = style.strict(&old) else {
        return Done::error(say(&texts.not_text, &[("path", path)])).said(said("edit/not-text"));
    };
    let places = match locate(texts, path, &text, &style, changes) {
        Ok(places) => places,
        Err(refused) => return refused,
    };
    let mut changed = text;
    for place in places.iter().rev() {
        changed.replace_range(place.range.clone(), &place.new);
    }
    let after = style.bytes(&changed);
    if stop.stopped() {
        return Done::stopped();
    }
    if let Err(error) = replace(&real, &after) {
        return texts.common.write_failed(path, &error);
    }
    Done::ok(say(&texts.edited, &[("path", &shown.path(&real))]))
        .said(said("edit/edited").with("count", places.len().to_string()))
        .effect(Effect::Changed {
            path: real,
            before: Some(old),
            after,
        })
}

/// 在原文 `text` 里找每一处，照原文里的先后排好；有一处出错、两处重叠的，交回不改的结果。
fn locate(
    texts: &Texts,
    path: &str,
    text: &str,
    style: &Style,
    changes: &[Change],
) -> Result<Vec<Place>, Done> {
    let mut places = Vec::new();
    for (index, change) in (1..).zip(changes) {
        let number = index.to_string();
        if change.old_string.is_empty() {
            return Err(Done::error(say(&texts.empty, &[("index", &number)]))
                .said(said("edit/empty").with("index", number)));
        }
        if change.old_string == change.new_string {
            return Err(Done::error(say(&texts.same, &[("index", &number)]))
                .said(said("edit/same").with("index", number)));
        }
        match find::find(text, &change.old_string, change.replace_all) {
            Found::At(ranges) => {
                let new = style.lines(&change.new_string);
                places.extend(ranges.into_iter().map(|range| Place {
                    range,
                    index,
                    new: new.clone(),
                }));
            }
            Found::Many { count, lines } => {
                let lines: Vec<String> = lines.iter().map(ToString::to_string).collect();
                let fields = [
                    ("index", number.as_str()),
                    ("path", path),
                    ("count", &count.to_string()),
                    ("lines", &lines.join(", ")),
                ];
                return Err(Done::error(say(&texts.not_unique, &fields)).said(
                    said("edit/not-unique")
                        .with("index", number.as_str())
                        .with("count", count.to_string()),
                ));
            }
            Found::Missing(closest) => {
                let mut message = say(&texts.not_found, &[("index", &number), ("path", path)]);
                let human = match closest {
                    Some(closest) => {
                        let (from, to) = (closest.from.to_string(), closest.to.to_string());
                        message.push_str(&say(&texts.closest, &[("from", &from), ("to", &to)]));
                        message.push_str(&closest.text);
                        said("edit/not-found-near")
                            .with("index", number.as_str())
                            .with("line", from)
                    }
                    None => said("edit/not-found").with("index", number.as_str()),
                };
                return Err(Done::error(message).said(human));
            }
        }
    }
    places.sort_by_key(|place| place.range.start);
    for pair in places.windows(2) {
        if pair[0].range.end > pair[1].range.start {
            let (first, second) = (
                pair[0].index.min(pair[1].index).to_string(),
                pair[0].index.max(pair[1].index).to_string(),
            );
            let fields = [
                ("first", first.as_str()),
                ("second", &second),
                ("path", path),
            ];
            return Err(Done::error(say(&texts.overlap, &fields)).said(
                said("edit/overlap")
                    .with("first", first.as_str())
                    .with("second", second.as_str()),
            ));
        }
    }
    Ok(places)
}
