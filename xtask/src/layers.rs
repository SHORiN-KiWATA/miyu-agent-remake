//! 分层门禁：依赖只朝一个方向（01-架构 第九节，规则 1、2、3、5）。

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::drawing::Drawing;

/// 工作区里的一个 crate，以及它依赖了谁。
pub struct Package {
    pub name: String,
    pub dir: PathBuf,
    pub deps: Vec<Dep>,
}

pub struct Dep {
    pub name: String,
    /// 开发依赖只给测试用，门禁不查。
    pub dev: bool,
}

/// 用 `cargo metadata` 读出工作区里的全部 crate。
pub fn read_packages(root: &Path, cargo: &str) -> Result<Vec<Package>, String> {
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("跑不了 cargo metadata：{e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata 失败：{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata 的输出读不懂：{e}"))?;
    let packages = json["packages"]
        .as_array()
        .ok_or("cargo metadata 的输出里没有 packages")?;
    packages.iter().map(package).collect()
}

fn package(json: &Value) -> Result<Package, String> {
    let name = json["name"].as_str().ok_or("有个 crate 没有名字")?;
    let manifest = json["manifest_path"]
        .as_str()
        .ok_or_else(|| format!("`{name}` 没有 manifest_path"))?;
    let dir = Path::new(manifest)
        .parent()
        .ok_or_else(|| format!("`{name}` 的 manifest_path 不对：{manifest}"))?
        .to_path_buf();
    let deps = json["dependencies"]
        .as_array()
        .map(|deps| {
            deps.iter()
                .filter_map(|dep| {
                    Some(Dep {
                        name: dep["name"].as_str()?.to_string(),
                        dev: dep["kind"].as_str() == Some("dev"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Package {
        name: name.to_string(),
        dir,
        deps,
    })
}

/// 查出全部违规，每条是一句说清哪里错了的话。
pub fn check(drawing: &Drawing, packages: &[Package]) -> Vec<String> {
    let mut problems = Vec::new();
    let in_workspace = |name: &str| packages.iter().any(|p| p.name == name);

    for layer in &drawing.layers {
        for name in &layer.crates {
            if !in_workspace(name) {
                problems.push(format!(
                    "图纸第 {} 层登记了 `{name}`，仓库里却没有这个 crate（规则 2）",
                    layer.level
                ));
            }
        }
    }
    for name in &drawing.tools {
        if !in_workspace(name) {
            problems.push(format!(
                "图纸登记了工具 crate `{name}`，仓库里却没有它（规则 2）"
            ));
        }
    }

    for package in packages {
        if drawing.is_tool(&package.name) {
            continue;
        }
        let Some(layer) = drawing.layer_of(&package.name) else {
            problems.push(format!(
                "`{}` 没有登记：在图纸的表里给它找一层（规则 2）",
                package.name
            ));
            continue;
        };
        for dep in package.deps.iter().filter(|d| !d.dev) {
            if in_workspace(&dep.name) {
                if drawing.is_tool(&dep.name) {
                    problems.push(format!(
                        "`{}` 依赖了工具 crate `{}`：工具 crate 谁都不许依赖（规则 5）",
                        package.name, dep.name
                    ));
                } else if let Some(dep_layer) = drawing.layer_of(&dep.name)
                    && dep_layer.level > layer.level
                {
                    problems.push(format!(
                        "`{}`（第 {} 层 {}）依赖了 `{}`（第 {} 层 {}）：低层不许依赖高层（规则 1）",
                        package.name,
                        layer.level,
                        layer.name,
                        dep.name,
                        dep_layer.level,
                        dep_layer.name
                    ));
                }
            } else if layer.pure && !drawing.whitelist.contains(&dep.name) {
                problems.push(format!(
                    "`{}` 在纯逻辑的第 {} 层，依赖了 `{}`：它不在图纸的白名单里（规则 3）",
                    package.name, layer.level, dep.name
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::Layer;

    fn drawing() -> Drawing {
        let layer = |level: u32, name: &str, pure: bool, crates: &[&str]| Layer {
            level,
            name: name.into(),
            pure,
            crates: crates.iter().map(|c| c.to_string()).collect(),
        };
        Drawing {
            layers: vec![
                layer(1, "内核", true, &["miyu-kernel"]),
                layer(2, "模块", true, &["miyu-prompt"]),
                layer(3, "执行器", false, &["miyu-store"]),
            ],
            whitelist: vec!["serde".into()],
            max_lines: 500,
            tools: vec!["xtask".into()],
        }
    }

    fn package(name: &str, deps: &[(&str, bool)]) -> Package {
        Package {
            name: name.into(),
            dir: PathBuf::from(name),
            deps: deps
                .iter()
                .map(|(dep, dev)| Dep {
                    name: dep.to_string(),
                    dev: *dev,
                })
                .collect(),
        }
    }

    fn workspace(extra: Vec<Package>) -> Vec<Package> {
        let mut packages = vec![
            package("miyu-kernel", &[]),
            package("miyu-prompt", &[("miyu-kernel", false)]),
            package("miyu-store", &[("miyu-kernel", false)]),
            package("xtask", &[("serde_json", false)]),
        ];
        for p in extra {
            packages.retain(|q| q.name != p.name);
            packages.push(p);
        }
        packages
    }

    #[test]
    fn a_workspace_that_follows_the_drawing_passes() {
        assert_eq!(check(&drawing(), &workspace(vec![])), Vec::<String>::new());
    }

    #[test]
    fn pure_crate_depending_on_tokio_is_rejected() {
        let packages = workspace(vec![package("miyu-kernel", &[("tokio", false)])]);
        let problems = check(&drawing(), &packages);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("`tokio`") && problems[0].contains("白名单"));
    }

    #[test]
    fn whitelisted_dependency_is_allowed() {
        let packages = workspace(vec![package("miyu-kernel", &[("serde", false)])]);
        assert!(check(&drawing(), &packages).is_empty());
    }

    #[test]
    fn io_crate_may_use_any_external_crate() {
        let packages = workspace(vec![package("miyu-store", &[("tokio", false)])]);
        assert!(check(&drawing(), &packages).is_empty());
    }

    #[test]
    fn lower_layer_depending_on_higher_layer_is_rejected() {
        let packages = workspace(vec![package("miyu-kernel", &[("miyu-store", false)])]);
        let problems = check(&drawing(), &packages);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("低层不许依赖高层"));
    }

    #[test]
    fn unregistered_crate_is_rejected() {
        let packages = workspace(vec![package("miyu-extra", &[])]);
        let problems = check(&drawing(), &packages);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("`miyu-extra` 没有登记"));
    }

    #[test]
    fn crate_in_drawing_but_missing_is_rejected() {
        let mut packages = workspace(vec![]);
        packages.retain(|p| p.name != "miyu-store");
        let problems = check(&drawing(), &packages);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("仓库里却没有"));
    }

    #[test]
    fn depending_on_tool_crate_is_rejected() {
        let packages = workspace(vec![package("miyu-store", &[("xtask", false)])]);
        let problems = check(&drawing(), &packages);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("工具 crate"));
    }

    #[test]
    fn dev_dependencies_are_not_checked() {
        let packages = workspace(vec![package(
            "miyu-kernel",
            &[("proptest", true), ("miyu-store", true)],
        )]);
        assert!(check(&drawing(), &packages).is_empty());
    }
}
