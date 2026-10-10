//! 清单里给头看的几格（施工 F-6 上，`docs/blueprint/package-pages.md`「清单多的几格」，设计 `30-插件框架.md` 第十三节）：
//! `[package] icon` 是 Lucide 的图标名，`[page] dir` 是软件后台页在包目录里的哪个子目录。只查写法：图标名在 Lucide 里有没有、
//! 目录里有没有 `index.html`，核心不管（头认不出的图标照没写画，没有入口的 `package.list` 不带 `page`）。

use toml_edit::{Item, TableLike};

use super::reader::{Reader, relative_dir};
use super::{Code, PackageKind, Problem};

/// 图标名最多几个字符。
const ICON_MAX: usize = 64;

/// `[package] icon`：小写字母开头，只有小写字母、数字、`-`，最多 64 个。
pub(super) fn icon(reader: &Reader<'_>, item: &Item) -> Result<String, Problem> {
    item.as_str()
        .filter(|name| icon_name(name))
        .map(str::to_string)
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadIcon,
                "package.icon",
                format!("package.icon must be a Lucide icon name: a lowercase letter, then lowercase letters, digits and -, at most {ICON_MAX}"),
            )
        })
}

/// `[page]`：只有 `dir`，必写，包目录里的相对目录（写法同 `[ui] pages_dir`）。
pub(super) fn page(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
) -> Result<String, Problem> {
    reader.only(table, "page", &["dir"])?;
    let item = reader.required(table, at, "page", "dir")?;
    item.as_str()
        .filter(|dir| relative_dir(dir))
        .map(str::to_string)
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadPageDir,
                "page.dir",
                "page.dir must be a relative directory inside the package directory".to_string(),
            )
        })
}

/// `[mascot]`（施工 F-7）：只有 `model`，必写，包目录里的相对路径（写法同 `[page] dir`：不许 `..`、开头的 `/`）。
pub(super) fn mascot(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
) -> Result<super::Mascot, Problem> {
    reader.only(table, "mascot", &["model"])?;
    let item = reader.required(table, at, "mascot", "model")?;
    item.as_str()
        .filter(|model| relative_dir(model))
        .map(|model| super::Mascot {
            model: model.to_string(),
        })
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadMascotModel,
                "mascot.model",
                "mascot.model must be a relative path inside the package directory".to_string(),
            )
        })
}

/// `[package] protocol`：带程序的必写；只带吉祥物的只有数据、不说协议，写了报 `wrong_kind`（施工 F-7）。
pub(super) fn protocol(
    reader: &Reader<'_>,
    kind: PackageKind,
    table: &dyn TableLike,
    at: &Item,
) -> Result<Option<[u32; 2]>, Problem> {
    if kind == PackageKind::Mascot {
        return match table.get("protocol") {
            None => Ok(None),
            Some(item) => Err(reader.problem(
                Some(item),
                Code::WrongKind,
                "package.protocol",
                "package.protocol is only for a package with a program".to_string(),
            )),
        };
    }
    let item = reader.required(table, at, "package", "protocol")?;
    super::reader::protocol(item).map(Some).ok_or_else(|| {
        reader.problem(
            Some(item),
            Code::BadProtocol,
            "package.protocol",
            "package.protocol must be two non-negative integers [lowest, highest]".to_string(),
        )
    })
}

/// 图标名的写法。
fn icon_name(name: &str) -> bool {
    name.len() <= ICON_MAX
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests;
