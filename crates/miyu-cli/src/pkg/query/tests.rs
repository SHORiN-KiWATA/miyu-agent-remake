//! `info`、`files`、`owns`、`check` 印成的那几行（施工 F-8 下，`docs/blueprint/cli/pkg.md`「样子」）。

use serde_json::json;

use miyu_kernel::time::UtcOffset;

use super::{checked, files, info, owner, size};
use crate::language::Language;

#[test]
fn info_is_aligned_rows_of_what_people_need() {
    let at = UtcOffset::from_minutes(8 * 60).expect("合法");
    let home = json!({"package": "xpkg", "layer": "home", "version": "2.0", "files": 3,
                      "size": 1536, "installed": "2026-10-11T06:03:00.000Z", "source": "/w/xpkg"});
    assert_eq!(
        info(&home, "测试包", at, &Language::Chinese),
        [
            "名称      测试包",
            "版本      2.0",
            "大小      1.5 KiB，3 个文件",
            "安装时间  2026-10-11 14:03",
        ],
        "从哪装的不印"
    );
    let shipped = json!({"package": "net", "layer": "shipped", "files": 1, "size": 10});
    assert_eq!(
        info(&shipped, "Net", at, &Language::English),
        [
            "Name       Net",
            "Size       10 B, 1 file",
            "Installed  shipped with Miyu",
        ],
        "没写版本的不印那一行"
    );
}

#[test]
fn files_are_the_id_and_the_whole_path() {
    let dir = std::env::temp_dir().join("xpkg");
    let listed = json!({"dir": dir, "files": [{"path": "bin/a.txt"}, {"path": "package.toml"}]});
    assert_eq!(
        files("xpkg", &listed),
        [
            format!("xpkg {}", dir.join("bin").join("a.txt").display()),
            format!("xpkg {}", dir.join("package.toml").display()),
        ]
    );
}

#[test]
fn owns_tells_installed_from_written_later() {
    let zh = Language::Chinese;
    let recorded = json!({"package": "xpkg", "layer": "home", "path": "a", "recorded": true});
    assert_eq!(
        owner("/p/a", &recorded, &zh).as_deref(),
        Some("/p/a 属于 xpkg")
    );
    let later = json!({"package": "xpkg", "layer": "home", "path": "c", "recorded": false});
    assert_eq!(
        owner("/p/c", &later, &zh).as_deref(),
        Some("/p/c 在 xpkg 的目录里，安装时没有")
    );
    let shipped = json!({"package": "net", "layer": "shipped", "path": "a", "recorded": false});
    assert_eq!(
        owner("/r/a", &shipped, &zh).as_deref(),
        Some("/r/a 属于 net")
    );
    assert_eq!(owner("/x", &json!({"package": null}), &zh), None);
}

#[test]
fn check_lists_what_changed_and_only_changes_and_losses_fail() {
    let zh = Language::Chinese;
    let found = json!({"packages": [
        {"package": "a", "modified": ["x"], "missing": ["y"], "extra": ["z"]},
        {"package": "b", "modified": [], "missing": [], "extra": []},
    ]});
    assert_eq!(
        checked(&found, &zh),
        (
            vec![
                "a：已修改 x".to_string(),
                "a：缺失 y".to_string(),
                "a：多出 z".to_string(),
                "b：正常".to_string(),
            ],
            true
        )
    );
    let extra =
        json!({"packages": [{"package": "a", "modified": [], "missing": [], "extra": ["z"]}]});
    assert_eq!(
        checked(&extra, &Language::English),
        (vec!["a: extra z".to_string()], false)
    );
    assert_eq!(
        checked(&json!({"packages": []}), &zh),
        (vec!["没有可检查的软件包".to_string()], false)
    );
}

#[test]
fn sizes_read_like_pacman() {
    assert_eq!(size(0), "0 B");
    assert_eq!(size(1023), "1023 B");
    assert_eq!(size(1024), "1.0 KiB");
    assert_eq!(size(1_572_864), "1.5 MiB");
    assert_eq!(size(5 * 1024 * 1024 * 1024 * 1024), "5120.0 GiB");
}
