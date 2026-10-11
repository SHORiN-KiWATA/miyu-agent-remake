//! 装、卸之前印的那一份、问的那一句（施工 F-8 下补，`docs/blueprint/cli/pkg.md`）。

use serde_json::json;

use super::{asked, plan};
use crate::language::{Doing, Language};

#[test]
fn an_upgrade_says_what_it_replaces_carries_needs_and_weighs() {
    let shown = json!({
        "package": "pudding", "version": "1.2.0", "replaces": {"version": "1.1.0"},
        "program": "process", "command": "pudding", "page": true, "mascot": true,
        "connection": "qq", "system_account": true, "settings": 2,
        "capabilities": [{"id": "network", "name": "联网", "summary": null},
                         {"id": "fs.read", "name": "读文件", "summary": null}],
        "files": 3, "size": 1536,
    });
    assert_eq!(
        plan(&shown, Doing::Install, &Language::Chinese),
        [
            "将安装 pudding 1.2.0（替换 1.1.0）",
            "包含：扩展程序、命令 miyu pudding、后台页、吉祥物、接入 qq、系统账号、2 项设置",
            "需要：联网、读文件",
            "大小：1.5 KiB，3 个文件",
        ]
    );
    let bare = json!({"package": "x", "replaces": {"version": null}, "files": 1, "size": 9});
    assert_eq!(
        plan(&bare, Doing::Install, &Language::English),
        [
            "Install x (replaces the installed one)",
            "Size: 9 B, 1 file"
        ],
        "没有的那一行不印"
    );
    let back = json!({"package": "net", "version": "1", "program": "builtin", "restores": true});
    assert_eq!(
        plan(&back, Doing::Restore, &Language::Chinese),
        ["将装回 net 1", "包含：内置功能"]
    );
}

#[test]
fn a_removal_says_what_goes_with_it() {
    let shown = json!({"package": "pudding", "layer": "home", "version": "1.2.0", "files": 3,
                       "size": 1536, "settings": ["pudding.port", "pudding.zones"], "state": true});
    assert_eq!(
        plan(&shown, Doing::Remove, &Language::Chinese),
        [
            "将卸载 pudding 1.2.0",
            "一并删除：设置 pudding.port、pudding.zones、状态目录",
            "大小：1.5 KiB，3 个文件",
        ]
    );
    let shipped = json!({"package": "net", "layer": "shipped", "settings": [], "state": false});
    assert_eq!(
        plan(&shipped, Doing::Remove, &Language::English),
        ["Remove net"]
    );
}

/// 答 `answer` 的结果、印在标准输出上的、印在标准错误上的。
fn answered(answer: &[u8], doing: Doing) -> (Result<(), u8>, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut input = answer;
    let result = asked(&mut input, &mut out, &mut err, doing, &Language::Chinese);
    let text = |bytes: Vec<u8>| String::from_utf8(bytes).expect("是 UTF-8");
    (result, text(out), text(err))
}

#[test]
fn enter_or_yes_goes_on_and_anything_else_or_nothing_stops() {
    for yes in [&b"\n"[..], b"y\n", b"Y\n", b"yes\n", b" YES \r\n"] {
        let (result, out, err) = answered(yes, Doing::Install);
        assert_eq!(result, Ok(()), "{yes:?}");
        assert_eq!((out.as_str(), err.as_str()), ("\n继续安装？[Y/n] ", ""));
    }
    for no in [&b"n\n"[..], b"no\n", b"x\n"] {
        let (result, _, err) = answered(no, Doing::Remove);
        assert_eq!((result, err.as_str()), (Err(1), "已取消\n"), "{no:?}");
    }
    let (result, out, err) = answered(b"", Doing::Remove);
    assert_eq!(result, Err(1), "标准输入关着的不做");
    assert_eq!(out, "\n继续卸载？[Y/n] \n");
    assert_eq!(err, "已取消：没有确认（不问用 --yes）\n");
}
