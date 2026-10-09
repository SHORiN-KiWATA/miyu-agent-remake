//! 一个软件包印成一行（`docs/blueprint/cli/pkg.md`「样子」）：`<编号>  <名字>`，照编号排（卸掉了的出厂的核心排在最后，这里
//! 照编号排进去），编号照最长的那个对齐；卸掉了的出厂的后面接「已卸载」，写错的没有名字、接它哪里不对。只放人要的：种类、哪一层、版本、功能不印（2026-10-08 项目主人定的设计原则）。

use serde_json::Value;

use crate::language::Language;

/// 核心回的那一串（`package.list` 的 `packages`），一个一行，不带换行。
pub(crate) fn listed(packages: &[Value], language: &Language) -> Vec<String> {
    let id = |package: &Value| package["package"].as_str().unwrap_or_default().to_string();
    let mut packages: Vec<&Value> = packages.iter().collect();
    packages.sort_by_key(|package| id(package));
    let width = packages
        .iter()
        .map(|package| id(package).chars().count())
        .max()
        .unwrap_or(0);
    packages
        .iter()
        .map(|package| {
            let package = *package;
            let name = package["name"].as_str().unwrap_or_default();
            let mut row = format!("{:width$}  {name}", id(package));
            if package["removed"].as_bool() == Some(true) {
                row.push_str(language.removed_mark());
            } else if let Some(problem) = package["problem"].as_str() {
                row.push_str(&language.broken_mark(problem));
            }
            row.trim_end().to_string()
        })
        .collect()
}
