//! `miyu pkg info`、`files`、`owns`、`check` 印成几行（施工 F-8 下，`docs/blueprint/cli/pkg.md`「样子」，照 pacman 的
//! `-Qi`、`-Ql`、`-Qo`、`-Qk`）：纯的，测试里照它算。只放人要的：从哪装的、哈希、哪一层不印（`--format json` 照原样有）。

use std::path::{Path, PathBuf};

use serde_json::Value;

use miyu_kernel::time::{Timestamp, UtcOffset};

use crate::config::columns;
use crate::language::{Checked, Language};

/// `info`：`info` 是 `package.info` 的回应，`name` 是 `package.list` 里它的名字（写错的包没有）。一行一格，前面的词照最宽的
/// 那个对齐；没写版本的、没记安装时间的那一行不印。
pub(crate) fn info(
    info: &Value,
    name: &str,
    offset: UtcOffset,
    language: &Language,
) -> Vec<String> {
    let [name_label, version_label, size_label, installed_label] = language.info_labels();
    let mut rows: Vec<(&str, String)> = Vec::new();
    if !name.is_empty() {
        rows.push((name_label, name.to_string()));
    }
    if let Some(version) = info["version"].as_str() {
        rows.push((version_label, version.to_string()));
    }
    let files = info["files"].as_u64().unwrap_or_default();
    let bytes = size(info["size"].as_u64().unwrap_or_default());
    rows.push((size_label, language.package_size(&bytes, files)));
    if info["layer"] == "shipped" {
        rows.push((installed_label, language.shipped().to_string()));
    } else if let Some(at) = info["installed"].as_str() {
        let at = Timestamp::parse(at).map_or_else(|_| at.to_string(), |at| at.local_minute(offset));
        rows.push((installed_label, at));
    }
    let width = rows
        .iter()
        .map(|(label, _)| columns(label))
        .max()
        .unwrap_or(0);
    rows.into_iter()
        .map(|(label, value)| {
            let pad = " ".repeat(width - columns(label));
            format!("{label}{pad}  {value}")
        })
        .collect()
}

/// `files`：一个文件一行，`<编号> <绝对路径>`（照 pacman 的 `-Ql`）。`files` 是 `package.files` 的回应。
pub(crate) fn files(id: &str, files: &Value) -> Vec<String> {
    let dir = PathBuf::from(files["dir"].as_str().unwrap_or_default());
    files["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|file| file["path"].as_str())
        .map(|path| format!("{id} {}", within(&dir, path).display()))
        .collect()
}

/// 包目录里 `/` 分开的相对路径接成这台机器的写法。
fn within(dir: &Path, path: &str) -> PathBuf {
    path.split('/')
        .fold(dir.to_path_buf(), |at, part| at.join(part))
}

/// `owns`：`path` 归哪个包。`owned` 是 `package.owns` 的回应；哪个包都没有的交回 `None`。家目录里装的、本地库没记它的
/// （包自己后来写的）另一种说法；出厂的不记，算它的。
pub(crate) fn owner(path: &str, owned: &Value, language: &Language) -> Option<String> {
    let id = owned["package"].as_str()?;
    let installed = owned["recorded"].as_bool() == Some(true) || owned["layer"] == "shipped";
    Some(match installed {
        true => language.owned_by(path, id),
        false => language.inside_of(path, id),
    })
}

/// `check`：每个包改了的、少了的、多出来的一行一个，都没问题的一行「正常」；家目录里一个记了的都没有的一行说没有。交回那几行，
/// 和有没有改了、少了的（有的退出码 1，照 pacman 的 `-Qk`；多出来的只是提示）。
pub(crate) fn checked(checked: &Value, language: &Language) -> (Vec<String>, bool) {
    let packages = checked["packages"].as_array().cloned().unwrap_or_default();
    if packages.is_empty() {
        return (vec![language.nothing_to_check().to_string()], false);
    }
    let mut rows = Vec::new();
    let mut broken = false;
    for package in &packages {
        let id = package["package"].as_str().unwrap_or_default();
        let before = rows.len();
        for (key, what) in [
            ("modified", Checked::Modified),
            ("missing", Checked::Missing),
            ("extra", Checked::Extra),
        ] {
            for path in package[key].as_array().into_iter().flatten() {
                rows.push(language.checked(id, what, path.as_str().unwrap_or_default()));
                broken |= what != Checked::Extra;
            }
        }
        if rows.len() == before {
            rows.push(language.checked(id, Checked::Fine, ""));
        }
    }
    (rows, broken)
}

/// 多少字节写成给人看的：不到 1 KiB 的写字节数，再往上一位小数的 KiB、MiB、GiB（照 pacman）。
pub(crate) fn size(bytes: u64) -> String {
    const UNITS: [&str; 3] = ["KiB", "MiB", "GiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    #[expect(clippy::cast_precision_loss, reason = "只给人看，一位小数")]
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests;
