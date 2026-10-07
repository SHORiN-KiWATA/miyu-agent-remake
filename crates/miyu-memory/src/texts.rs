//! 三件工具输出里给她看的几句（`software/memory/<工具>/*.txt`，几件都要说的在 `software/memory/common/`），和给人看的说法的
//! 编号（`software/memory/<key>`，字在 `software/memory/human/<语言>.json`）。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_recall::MemoryId;
use miyu_tool::load::{self, LoadError, say};
use miyu_tool::{Done, Refused};

use crate::PACKAGE;

/// 读好的几句，三件工具共用一份。
#[derive(Clone)]
pub(crate) struct Texts(Arc<All>);

pub(crate) struct All {
    pub(crate) bad_args: Template,
    pub(crate) off: Template,
    pub(crate) failed: Template,
    pub(crate) no_such: Template,
    pub(crate) not_current: Template,
    pub(crate) saved: Template,
    pub(crate) replaced: Template,
    pub(crate) too_long: Template,
    pub(crate) unknown_class: Template,
    pub(crate) retired: Template,
    pub(crate) memory: Template,
    pub(crate) memory_retired: Template,
    pub(crate) turn: Template,
    pub(crate) nothing: Template,
}

impl Texts {
    /// 照资源目录 `resources` 读，每一份拿它要的字段试换一次。
    pub(crate) fn load(resources: &Path) -> Result<Texts, LoadError> {
        let text = |tool: &str, name: &str, fields: &[&str]| {
            load::text(resources, PACKAGE, tool, name, fields)
        };
        Ok(Texts(Arc::new(All {
            bad_args: text("common", "bad-args", &["error"])?,
            off: text("common", "off", &[])?,
            failed: text("common", "failed", &["error"])?,
            no_such: text("common", "no-such", &["id"])?,
            not_current: text("common", "not-current", &["id"])?,
            saved: text("remember", "saved", &["id"])?,
            replaced: text("remember", "replaced", &["id", "old"])?,
            too_long: text("remember", "too-long", &["chars", "limit"])?,
            unknown_class: text("remember", "unknown-class", &["class"])?,
            retired: text("forget", "retired", &["id"])?,
            memory: text("memory_search", "memory", &["id", "class", "date", "text"])?,
            memory_retired: text("memory_search", "retired", &["id", "class", "date", "text"])?,
            turn: text("memory_search", "turn", &["date", "session", "text"])?,
            nothing: text("memory_search", "nothing", &[])?,
        })))
    }

    /// 读好的几句。
    pub(crate) fn all(&self) -> &All {
        &self.0
    }

    /// 一句出错：照 `template` 换进 `fields`，给人看的是 `key`、带同样的字段。
    pub(crate) fn error(&self, template: &Template, key: &str, fields: &[(&str, &str)]) -> Done {
        Done::error(line(template, fields)).said(said(key, fields))
    }

    /// 参数不对。
    pub(crate) fn bad_args(&self, error: &dyn std::fmt::Display) -> Done {
        self.error(
            &self.0.bad_args,
            "common/bad-args",
            &[("error", &error.to_string())],
        )
    }

    /// 这个会话没开记忆：没有端口。
    pub(crate) fn off(&self) -> Done {
        self.error(&self.0.off, "common/off", &[])
    }

    /// 没有这一条：她写的编号原样说（写法不对的也是）。
    pub(crate) fn no_such(&self, id: &str) -> Done {
        self.error(&self.0.no_such, "common/no-such", &[("id", id)])
    }

    /// 端口拒了：照哪一种说一句。
    pub(crate) fn refused(&self, refused: &Refused) -> Done {
        match refused {
            Refused::NoSuch(id) => self.no_such(&id.to_string()),
            Refused::NotCurrent(id) => self.error(
                &self.0.not_current,
                "common/not-current",
                &[("id", &id.to_string())],
            ),
            Refused::Failed(error) => {
                self.error(&self.0.failed, "common/failed", &[("error", error)])
            }
        }
    }
}

/// 照模板换进字段，去掉末尾的换行：几句接成几行时由调的一方加换行。
pub(crate) fn line(template: &Template, fields: &[(&str, &str)]) -> String {
    say(template, fields).trim_end().to_string()
}

/// 给人看的说法：编号是 `software/memory/<key>`，带着换进去的字段。
pub(crate) fn said(key: &str, fields: &[(&str, &str)]) -> Said {
    fields.iter().fold(
        Said::new(format!("software/{PACKAGE}/{key}")),
        |said, (name, value)| said.with(name, (*value).to_string()),
    )
}

/// 她写的编号：认不出的交回原样，好照「没有这一条」说。
pub(crate) fn memory_id(text: &str) -> Result<MemoryId, String> {
    MemoryId::parse(text).ok_or_else(|| text.to_string())
}
