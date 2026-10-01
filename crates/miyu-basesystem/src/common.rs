//! 几件工具都要用的（施工 4-4 下）：几件都要说的几句字（`26-提示词.md` 第八节，`software/basesystem/common/`），
//! 结果里的路径怎么写（[`Shown`]），找不到时同一个目录里相近的名字，当没传的几种写法（[`given`]）；改一个已经在了
//! 的文件之前核对她看过的（[`Common::unseen`]，施工 4-6 中从 `write` 挪来，`edit` 也用）；写成字符串的整数也认（[`integer()`]，
//! 施工 4-9 再补二）。

mod integer;
mod shown;
mod similar;

use std::fmt::Display;
use std::io::Read;
use std::path::Path;

use miyu_fs::{OpenError, open_file};

use miyu_kernel::event::Said;
use miyu_kernel::id::ContentHash;
use miyu_kernel::template::Template;
use miyu_tool::{Call, Done};

use crate::load::{self, LoadError, say};

pub(crate) use integer::integer;
pub(crate) use shown::Shown;

/// 一次的输出最多多少字节：`read`、`grep` 一样，到了就停在那一条，说从哪接。
pub(crate) const OUTPUT_BYTES: usize = 64 * 1024;

/// 几件工具都要说的几句。
#[derive(Clone)]
pub(crate) struct Common {
    missing: Template,
    similar: Template,
    failed: Template,
    bad_args: Template,
    bad_glob: Template,
    no_files: Template,
    not_read: Template,
    stale: Template,
    directory: Template,
    not_regular: Template,
    write_failed: Template,
}

impl Common {
    /// 照资源目录 `resources` 里的字造。
    pub(crate) fn load(resources: &Path) -> Result<Common, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "common", name, fields);
        Ok(Common {
            missing: text("missing", &["path"])?,
            similar: text("similar", &["path"])?,
            failed: text("failed", &["path", "error"])?,
            bad_args: text("bad-args", &["error"])?,
            bad_glob: text("bad-glob", &["glob", "error"])?,
            no_files: text("no-files", &[])?,
            not_read: text("not-read", &["path"])?,
            stale: text("stale", &["path"])?,
            directory: text("directory", &["path"])?,
            not_regular: text("not-a-regular-file", &["path"])?,
            write_failed: text("write-failed", &["path", "error"])?,
        })
    }

    /// 要写、要改的 `path` 是个目录。
    pub(crate) fn directory(&self, path: &str) -> Done {
        Done::error(say(&self.directory, &[("path", path)])).said(said("common/directory"))
    }

    /// 要写、要改的 `path` 不是普通文件：FIFO、设备这类。
    pub(crate) fn not_regular(&self, path: &str) -> Done {
        Done::error(say(&self.not_regular, &[("path", path)]))
            .said(said("common/not-a-regular-file"))
    }

    /// 写 `path` 的时候出错了，系统说的是 `error`。
    pub(crate) fn write_failed(&self, path: &str, error: &dyn Display) -> Done {
        let error = error.to_string();
        Done::error(say(
            &self.write_failed,
            &[("path", path), ("error", &error)],
        ))
        .said(said("common/write-failed").with("error", error))
    }

    /// 改一个已经在了的文件之前核对她看过的（`10-自带软件.md` 第五节「她看过的」）：她给的是 `path`，真实的位置
    /// 是 `real`，现在的内容是 `old`。她没看过的、看过以后又被改了的，交回不改的结果；对得上的是空的。
    pub(crate) fn unseen(&self, call: &Call, path: &str, real: &Path, old: &[u8]) -> Option<Done> {
        let (template, key) = match call.seen.get(real) {
            None => (&self.not_read, "common/not-read"),
            Some(hash) if *hash != ContentHash::of(old) => (&self.stale, "common/stale"),
            Some(_) => return None,
        };
        Some(Done::error(say(template, &[("path", path)])).said(said(key)))
    }

    /// 没有这个文件或目录：她给的是 `path`，换成的真实位置是 `real`。同一个目录里有相近的名字，一个一行列在后面，
    /// 照 `shown` 写；给人看的只带第一个。
    pub(crate) fn missing(&self, path: &str, real: &Path, shown: &Shown) -> Done {
        let mut text = say(&self.missing, &[("path", path)]);
        let near: Vec<String> = similar::names(real)
            .iter()
            .map(|near| shown.path(near))
            .collect();
        for near in &near {
            text.push_str(&say(&self.similar, &[("path", near)]));
        }
        let human = match near.first() {
            Some(first) => said("common/missing-similar")
                .with("path", path)
                .with("similar", first.as_str()),
            None => said("common/missing").with("path", path),
        };
        Done::error(text).said(human)
    }

    /// 读 `path` 的时候出错了，系统说的是 `error`。
    pub(crate) fn failed(&self, path: &str, error: &dyn Display) -> Done {
        let error = error.to_string();
        Done::error(say(&self.failed, &[("path", path), ("error", &error)])).said(
            said("common/failed")
                .with("path", path)
                .with("error", error),
        )
    }

    /// 参数不对。
    pub(crate) fn bad_args(&self, error: &dyn Display) -> Done {
        let error = error.to_string();
        Done::error(say(&self.bad_args, &[("error", &error)]))
            .said(said("common/bad-args").with("error", error))
    }

    /// 通配 `glob` 写得不对，`error` 是哪里不对。
    pub(crate) fn bad_glob(&self, glob: &str, error: &dyn Display) -> Done {
        let error = error.to_string();
        Done::error(say(&self.bad_glob, &[("glob", glob), ("error", &error)])).said(
            said("common/bad-glob")
                .with("glob", glob)
                .with("error", error),
        )
    }

    /// 一个文件都没找到：不算出错。
    pub(crate) fn no_files(&self) -> Done {
        Done::ok(say(&self.no_files, &[])).said(said("common/no-files"))
    }
}

/// 基础系统给人看的说法：编号是 `software/basesystem/` 下的 `key`（施工 4-5 上）。字在
/// `software/basesystem/human/<语言>.json` 里。
pub(crate) fn said(key: &str) -> Said {
    Said::new(format!("software/basesystem/{key}"))
}

/// 说法管着一个数、后面跟一个可数名词的那几句（施工 4-5 再补「一个的时候说单数」）：`n` 是 1 的时候，编号多接一段
/// `/one`，换出来的那一句写成单数；别的数照旧走 `key` 本身。换进去的字段还是叫 `field`，值是 `n`。
pub(crate) fn said_n(key: &str, field: &str, n: u64) -> Said {
    let route = if n == 1 {
        format!("{key}/one")
    } else {
        key.to_string()
    };
    said(&route).with(field, n.to_string())
}

/// 一个可选的字符串参数：`"undefined"`、`"null"`、空串当没传。Claude Code 在说明里专门叮嘱过模型别这么传，
/// 这里在代码里兜住，不写进说明（施工 4-4 下）。
pub(crate) fn given(value: Option<String>) -> Option<String> {
    value.filter(|value| !matches!(value.as_str(), "" | "undefined" | "null"))
}

#[cfg(test)]
mod tests;

/// 一个已经在了的文件整份的内容，照「安全地打开」开（施工 4-9 再补二）：不跟最后一层的链接、不阻塞，开了以后看是
/// 不是普通文件，没人写的 FIFO 也不卡住。没有的交回空的。`write`、`edit` 改之前照它读。
///
/// # Errors
///
/// 不是普通文件（目录、FIFO、设备……）；打不开、读不了。
pub(crate) fn contents(real: &Path) -> Result<Option<Vec<u8>>, OpenError> {
    let mut file = match open_file(real) {
        Ok(file) => file,
        Err(OpenError::NotFound) => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(OpenError::Io)?;
    Ok(Some(bytes))
}
