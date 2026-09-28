//! 门禁的「许可证」一项（施工 4-12，`docs/blueprint/licenses.md`）：仓库是 GPL-3.0-or-later，发布的四个平台上用得到的
//! 第三方依赖，许可证都要能和它合在一起发。照每个包 `license` 里的 SPDX 表达式算，自己算，不加依赖。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// 发布的四个平台（`docs/designs/12-进程形态与分发.md` R11）。
pub const PLATFORMS: [&str; 4] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

/// 能和 GPL-3.0-or-later 合在一起发的许可证。
const ALLOWED: [&str; 14] = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Unlicense",
    "CC0-1.0",
    "BSL-1.0",
    "MPL-2.0",
    "CDLA-Permissive-2.0",
];

/// 认得的例外条款：`WITH` 后面写它，不影响前面那个许可证。
const EXCEPTIONS: [&str; 1] = ["LLVM-exception"];

/// 一个第三方包：名字、版本，和它 `license` 的原文（没写的是空的）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) license: Option<String>,
}

/// 查一遍四个平台，交回报出来的问题。
pub fn check(root: &Path, cargo: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut problems = Vec::new();
    for platform in PLATFORMS {
        match packages(root, cargo, platform) {
            Ok(list) => found.extend(list.into_iter().map(|package| (package, platform))),
            Err(e) => problems.push(format!("跑不了 cargo metadata（{platform}）：{e}")),
        }
    }
    problems.extend(judge(&found));
    problems
}

/// 每个包说一句：许可证不能用的、没写的。同一个包在几个平台上都有，只说一次，写上是哪几个平台。
pub(crate) fn judge(found: &[(Package, &str)]) -> Vec<String> {
    let mut platforms: BTreeMap<&Package, BTreeSet<&str>> = BTreeMap::new();
    for (package, platform) in found {
        platforms.entry(package).or_default().insert(platform);
    }
    let mut problems = Vec::new();
    for (package, on) in platforms {
        let on: Vec<&str> = on.into_iter().collect();
        let head = format!("{} {}（{}）", package.name, package.version, on.join("、"));
        match &package.license {
            None => problems.push(format!("{head}：没写 license，要人看过")),
            Some(license) if !allowed(license) => {
                problems.push(format!("{head}：{license} 和 GPL-3.0-or-later 合不到一起"));
            }
            Some(_) => {}
        }
    }
    problems
}

/// 一个平台上用得到的第三方包。
fn packages(root: &Path, cargo: &str, platform: &str) -> Result<Vec<Package>, String> {
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--locked"])
        .args(["--filter-platform", platform])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(said.lines().next().unwrap_or_default().to_string());
    }
    let json: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    Ok(third_party(&json))
}

/// 从 `cargo metadata` 的输出里取：依赖图里用得到的包，去掉工作区自己的。
pub(crate) fn third_party(json: &Value) -> Vec<Package> {
    let strings = |value: &Value| -> BTreeSet<String> {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect()
    };
    let members = strings(&json["workspace_members"]);
    let used: BTreeSet<String> = json["resolve"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|node| node["id"].as_str().map(str::to_string))
        .collect();
    json["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|package| {
            package["id"]
                .as_str()
                .is_some_and(|id| used.contains(id) && !members.contains(id))
        })
        .map(|package| Package {
            name: package["name"].as_str().unwrap_or_default().to_string(),
            version: package["version"].as_str().unwrap_or_default().to_string(),
            license: package["license"].as_str().map(str::to_string),
        })
        .collect()
}

/// 一个 SPDX 表达式能不能用：`OR` 有一个能用就行（老写法的 `/` 也是 `OR`），`AND` 每个都要能用，括号照括号，
/// `WITH` 后面只认 [`EXCEPTIONS`]。写坏了的当不能用。
pub(crate) fn allowed(expression: &str) -> bool {
    let spaced = expression
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace('/', " OR ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut at = 0;
    match any(&tokens, &mut at) {
        Some(ok) => ok && at == tokens.len(),
        None => false,
    }
}

/// `OR` 连着的几段：有一段能用就行。写坏了的是空的。
fn any(tokens: &[&str], at: &mut usize) -> Option<bool> {
    let mut ok = all(tokens, at)?;
    while tokens
        .get(*at)
        .is_some_and(|t| t.eq_ignore_ascii_case("OR"))
    {
        *at += 1;
        ok |= all(tokens, at)?;
    }
    Some(ok)
}

/// `AND` 连着的几段：每一段都要能用。
fn all(tokens: &[&str], at: &mut usize) -> Option<bool> {
    let mut ok = one(tokens, at)?;
    while tokens
        .get(*at)
        .is_some_and(|t| t.eq_ignore_ascii_case("AND"))
    {
        *at += 1;
        ok &= one(tokens, at)?;
    }
    Some(ok)
}

/// 一个许可证（可以带 `WITH` 例外），或者括号里的一整段。
fn one(tokens: &[&str], at: &mut usize) -> Option<bool> {
    let token = *tokens.get(*at)?;
    *at += 1;
    let ok = match token {
        "(" => {
            let ok = any(tokens, at)?;
            (tokens.get(*at) == Some(&")")).then_some(())?;
            *at += 1;
            ok
        }
        ")" => return None,
        _ if ["AND", "OR", "WITH"]
            .iter()
            .any(|op| token.eq_ignore_ascii_case(op)) =>
        {
            return None;
        }
        id => ALLOWED.contains(&id.trim_end_matches('+')),
    };
    if tokens
        .get(*at)
        .is_some_and(|t| t.eq_ignore_ascii_case("WITH"))
    {
        let exception = *tokens.get(*at + 1)?;
        *at += 2;
        return Some(ok && EXCEPTIONS.contains(&exception));
    }
    Some(ok)
}

#[cfg(test)]
mod tests;
