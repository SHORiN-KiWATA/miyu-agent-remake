//! 功能平铺、切一行写哪几个键（「配置页」第 38 条）。

use serde_json::json;

use super::{Mark, Row, apply, mark, offs, read, rows, selectable, toggle, toggle_all};

/// 文件读写（几件工具）、运行命令（一件）、人设防失忆提醒（没有工具）、没装的联网。
fn sample() -> Vec<super::Feature> {
    read(&json!({"features": [
        {"id": "files", "name": "文件读写", "on": true, "installed": true, "tools": [
            {"name": "glob", "label": "找文件", "on": true},
            {"name": "grep", "label": "搜内容", "on": false},
            {"name": "read", "label": "读取", "on": true}]},
        {"id": "commands", "name": "运行命令", "on": false, "installed": true, "tools": [
            {"name": "shell", "label": "执行命令", "on": false}]},
        {"id": "roleplay", "name": "人设防失忆提醒", "on": true, "installed": true, "tools": []},
        {"id": "web", "name": "联网", "on": true, "installed": false, "tools": []}]}))
}

#[test]
fn features_lie_flat_with_their_tools_under_them() {
    // 2026-10-09 项目主人：「每一个工具都是一个功能，直接平铺，然后用组名做分隔线」。
    let features = sample();
    assert_eq!(
        rows(&features),
        [
            Row::Feature(0),
            Row::Tool(0, 0),
            Row::Tool(0, 1),
            Row::Tool(0, 2),
            Row::Feature(1),
            Row::Feature(2),
            Row::Feature(3)
        ],
        "只有一件、没有工具的只有功能那一行"
    );
    assert_eq!(mark(&features[0]), Mark::Some, "关了一件工具");
    assert_eq!(mark(&features[1]), Mark::Off);
    assert_eq!(mark(&features[2]), Mark::On);
    assert!(!selectable(&features, Row::Feature(3)), "没装的切不了");
}

#[test]
fn space_writes_the_feature_or_the_tool_and_ctrl_a_flips_everything() {
    let features = sample();
    assert_eq!(
        toggle(&features, Row::Tool(0, 1)),
        [("tools.grep".to_string(), true)]
    );
    assert_eq!(
        toggle(&features, Row::Feature(0)),
        [("features.files".to_string(), false)]
    );
    // 功能关着时切它的工具 = 打开这个功能。
    let mut off = features.clone();
    off[0].on = false;
    assert_eq!(
        toggle(&off, Row::Tool(0, 0)),
        [("features.files".to_string(), true)]
    );
    assert!(toggle(&features, Row::Feature(3)).is_empty(), "没装的不写");
    // 有没开的：全开（关着的功能、开着的功能里关掉的工具）。
    let all_on = toggle_all(&features);
    assert_eq!(
        all_on,
        [
            ("tools.grep".to_string(), true),
            ("features.commands".to_string(), true)
        ]
    );
    let mut on = features.clone();
    apply(&mut on, &all_on);
    assert!(
        on[1].on && on[0].tools.iter().all(|t| t.on),
        "照写的键改手上的"
    );
    // 核心那边打开功能以后它的工具回到各自原来的开关：这里当都开着。
    on[1].tools[0].on = true;
    assert!(offs(&on).is_empty(), "都开着：新建什么都不用写");
    // 都开着：全关功能。
    assert_eq!(
        toggle_all(&on),
        [
            ("features.files".to_string(), false),
            ("features.commands".to_string(), false),
            ("features.roleplay".to_string(), false)
        ]
    );
    assert_eq!(
        offs(&features),
        [
            ("tools.grep".to_string(), false),
            ("features.commands".to_string(), false)
        ]
    );
}
