//! 读的三件工具都要用的（施工 4-4 下）：几件都要说的几句字（`26-提示词.md` 第八节，`software/basesystem/common/`），
//! 结果里的路径怎么写（[`Shown`]），找不到时同一个目录里相近的名字，当没传的几种写法（[`given`]）。

mod shown;
mod similar;

use std::fmt::Display;
use std::path::Path;

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_tool::Done;

use crate::load::{self, LoadError, say};

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
        })
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

/// 一个可选的字符串参数：`"undefined"`、`"null"`、空串当没传。Claude Code 在说明里专门叮嘱过模型别这么传，
/// 这里在代码里兜住，不写进说明（施工 4-4 下）。
pub(crate) fn given(value: Option<String>) -> Option<String> {
    value.filter(|value| !matches!(value.as_str(), "" | "undefined" | "null"))
}

#[cfg(test)]
mod tests;
