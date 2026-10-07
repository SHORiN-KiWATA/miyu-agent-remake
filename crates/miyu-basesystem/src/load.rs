//! 从资源目录读基础系统的工具的字（`26-提示词.md` 第八节）：`software/basesystem/tools/<工具>.json` 是给模型看的说明和
//! 参数格式，`software/basesystem/<工具>/<名字>.txt` 是输出里给她看的几句，几件工具都要说的在
//! `software/basesystem/common/` 下（施工 4-4 下）。怎么读在 `miyu_tool::load`（施工 R-3 中挪过去，记忆这个软件包也照它读），
//! 这里只填上软件包名。

use std::path::Path;

use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::Spec;
use miyu_tool::load;

pub use miyu_tool::load::{LoadError, say};

/// 这个软件包在资源目录里的名字。
const PACKAGE: &str = "basesystem";

/// 读工具 `name` 的说明和参数格式，访问类别是 `access`。
pub(crate) fn spec(resources: &Path, name: &str, access: Access) -> Result<Spec, LoadError> {
    load::spec(resources, PACKAGE, name, access)
}

/// 同 [`spec`]，说明是一段模板，换进 `fields`（`shell` 写用的是哪种 shell，施工 4-8）。
pub(crate) fn spec_filled(
    resources: &Path,
    name: &str,
    access: Access,
    fields: &[(&str, &str)],
) -> Result<Spec, LoadError> {
    load::spec_filled(resources, PACKAGE, name, access, fields)
}

/// 读工具 `tool` 输出里的一句 `name`，拿 `fields` 里的每个字段试换一次。
pub(crate) fn text(
    resources: &Path,
    tool: &str,
    name: &str,
    fields: &[&str],
) -> Result<Template, LoadError> {
    load::text(resources, PACKAGE, tool, name, fields)
}
