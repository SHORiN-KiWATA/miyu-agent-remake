//! `miyu pkg` 印的那一行、装的是清单还是编号（施工 T-3，`docs/blueprint/cli/pkg.md`）。

use std::path::Path;

use serde_json::json;

use super::{installing, shown};
use crate::language::Language;

#[test]
fn a_package_is_one_aligned_line_and_only_what_people_need() {
    let packages = [
        json!({"package": "basesystem", "layer": "shipped", "kind": "builtin", "name": "基础系统", "version": "1"}),
        json!({"package": "net", "layer": "shipped", "kind": "builtin", "name": "联网", "removed": true}),
        json!({"package": "bad", "layer": "home", "code": "bad_toml", "problem": "第一行不是 TOML"}),
    ];
    assert_eq!(
        shown::listed(&packages, &Language::Chinese),
        [
            "bad         （写错了：第一行不是 TOML）",
            "basesystem  基础系统",
            "net         联网（已卸载）",
        ],
        "照编号排"
    );
    assert_eq!(
        shown::listed(&packages[1..2], &Language::English),
        ["net  联网 (removed)"]
    );
}

#[test]
fn a_path_is_a_package_folder_and_a_bare_word_is_a_package() {
    // 真的绝对路径：Windows 上 `/work` 不算绝对的，拼出来的样子两边不一样。
    let temp = std::env::temp_dir();
    let cwd = temp.join("work");
    let cwd = cwd.as_path();
    assert_eq!(installing("./x", cwd), json!({"path": cwd.join("x")}));
    assert_eq!(installing("x/", cwd), json!({"path": cwd.join("x")}));
    assert_eq!(installing(".", cwd), json!({"path": cwd}));
    assert_eq!(installing("../y", cwd), json!({"path": temp.join("y")}));
    assert_eq!(
        installing("package.toml", cwd),
        json!({"path": cwd.join("package.toml")})
    );
    let elsewhere = temp.join("abs").join("y");
    assert_eq!(
        installing(&elsewhere.to_string_lossy(), cwd),
        json!({"path": elsewhere})
    );
    assert_eq!(installing("net", cwd), json!({"package": "net"}));
}
