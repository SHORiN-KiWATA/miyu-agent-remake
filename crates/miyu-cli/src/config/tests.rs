//! `miyu config` 印的样子（`config.md`「样子」）：`get` 的值、报错一行、`check` 的合计、`explain` 的几行照显示的宽度对齐，
//! 两种语言；核心报的文件换成真的位置、家目录写成 `~/…`；还没有项目配置时它该在哪。

use std::path::PathBuf;

use serde_json::json;

use super::paths::{Places, planned};
use super::render::{columns, explain, problem, toml, values};
use super::{ConfigCommand, ConfigPlan};
use crate::ask::Format;
use crate::language::Language;

/// 假的家目录：绝对路径，三个平台都是（`/home/me` 在 Windows 上不算绝对路径）。
fn home() -> PathBuf {
    std::env::temp_dir().join("me")
}

fn places() -> Places {
    Places::of(&ConfigPlan {
        command: ConfigCommand::Get {
            keys: Vec::new(),
            format: Format::Text,
        },
        language: Language::Chinese,
        cwd: home().join("work"),
        root: home().join(".miyu"),
        home: Some(home()),
        color: false,
    })
}

#[test]
fn get_prints_one_bare_value_or_every_key_as_toml() {
    let items = json!({
        "log.level": {"origin": {"layer": "default"}, "value": "info"},
        "permission.start_read_only": {"origin": {"layer": "default"}, "value": false},
        "ui.language": {"origin": {"layer": "default"}, "value": "zh"},
    });
    assert_eq!(
        values(&items, false),
        "log.level = \"info\"\npermission.start_read_only = false\nui.language = \"zh\"\n"
    );
    let one = json!({"ui.language": {"value": "zh"}});
    assert_eq!(values(&one, true), "zh\n", "只写一个键的只印值，字不带引号");
    let switch = json!({"permission.start_read_only": {"value": true}});
    assert_eq!(values(&switch, true), "true\n");
}

#[test]
fn values_are_written_as_toml() {
    assert_eq!(toml(&json!("a\"b")), "\"a\\\"b\"");
    assert_eq!(toml(&json!(3)), "3");
    assert_eq!(toml(&json!(["a", "b"])), "[\"a\", \"b\"]");
}

#[test]
fn a_problem_line_starts_with_path_line_and_column() {
    let found = json!({"code":"unknown_key","column":1,"level":"warning","line":7,
        "message":"没有 ui.langauge 这一项。是不是想写 ui.language？这一行先不管，原样留着。"});
    let file = "~/.miyu/home/admin/settings.toml";
    assert_eq!(
        problem(file, &found, Language::Chinese).paint(false),
        "~/.miyu/home/admin/settings.toml:7:1 警告：没有 ui.langauge 这一项。是不是想写 ui.language？这一行先不管，原样留着。\n"
    );
    let english = json!({"level":"error","line":2,"column":9,"message":"Not valid TOML."});
    assert_eq!(
        problem("~/.miyu/system/config.toml", &english, Language::English).paint(false),
        "~/.miyu/system/config.toml:2:9 error: Not valid TOML.\n"
    );
    let whole = json!({"level":"error","message":"这份文件超过 1 MiB，不读。"});
    assert_eq!(
        problem("~/a.toml", &whole, Language::Chinese).paint(false),
        "~/a.toml 错误：这份文件超过 1 MiB，不读。\n",
        "整份的问题没有行列"
    );
    assert_eq!(
        problem("~/a.toml", &whole, Language::Chinese).paint(true),
        "~/a.toml \u{1b}[31m错误\u{1b}[0m：这份文件超过 1 MiB，不读。\u{1b}[0m\n",
        "错误红"
    );
    assert!(
        problem(file, &found, Language::English)
            .paint(true)
            .contains("\u{1b}[33mwarning"),
        "警告黄"
    );
}

#[test]
fn the_total_leaves_out_what_there_is_none_of() {
    assert_eq!(Language::Chinese.problems_total(2, 1), "2 处错误，1 处警告");
    assert_eq!(Language::Chinese.problems_total(0, 3), "3 处警告");
    assert_eq!(Language::Chinese.problems_total(0, 0), "没有问题");
    assert_eq!(
        Language::English.problems_total(2, 1),
        "2 errors, 1 warning"
    );
    assert_eq!(Language::English.problems_total(1, 0), "1 error");
    assert_eq!(Language::English.problems_total(0, 0), "No problems");
}

/// `config.get` 带 `all` 的一项和 `config.schema` 的一项：图纸「样子」里 `ui.language` 的例子。
fn language_item() -> (serde_json::Value, serde_json::Value) {
    let item = json!({
        "layers": [
            {"origin": {"file": "home/admin/settings.toml", "layer": "personal", "line": 3}, "used": true, "value": "zh"},
            {"origin": {"file": "system/config.toml", "layer": "system", "line": 5}, "used": false, "value": "en"},
            {"origin": {"layer": "default"}, "used": false, "value": "auto"},
        ],
        "origin": {"file": "home/admin/settings.toml", "layer": "personal", "line": 3},
        "value": "zh",
    });
    let said = |name: &str, description: &str| json!({"applies": "now", "description": description, "key": "ui.language", "name": name});
    let chinese = said(
        "界面语言",
        "终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。",
    );
    (item, chinese)
}

#[test]
fn explain_lines_up_the_layers_like_the_blueprint() {
    let (item, said) = language_item();
    let lines: Vec<String> = explain("ui.language", &item, &said, Language::Chinese, &places())
        .iter()
        .map(|line| line.paint(false))
        .collect();
    // 路径照平台的分隔符写（`~/…` 在 Windows 上是 `~\…`），别的一字不差。
    let (personal, system) = (
        places().shown("home/admin/settings.toml"),
        places().shown("system/config.toml"),
    );
    assert_eq!(
        lines.concat(),
        format!(
            "界面语言（ui.language）：终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。当场生效。\n\
             \x20 \"zh\"    个人设置  {personal}:3  ← 生效\n\
             \x20 \"en\"    系统配置  {system}:5\n\
             \x20 \"auto\"  默认值\n"
        )
    );
    let english = json!({"applies": "now", "name": "Interface language",
        "description": "The language terminals, the web page and the command line use for you. auto follows the terminal or browser."});
    let lines: Vec<String> = explain("ui.language", &item, &english, Language::English, &places())
        .iter()
        .map(|line| line.paint(false))
        .collect();
    assert_eq!(
        lines.concat(),
        format!(
            "Interface language (ui.language): The language terminals, the web page and the command line use for you. auto follows the terminal or browser. Takes effect at once.\n\
             \x20 \"zh\"    personal settings  {personal}:3  ← in effect\n\
             \x20 \"en\"    system config      {system}:5\n\
             \x20 \"auto\"  default\n"
        )
    );
}

#[test]
fn explain_marks_the_environment_and_what_does_not_count() {
    let item = json!({"layers": [
        {"origin": {"layer": "env", "name": "MIYU_LOG"}, "used": true, "value": "debug"},
        {"origin": {"file": "~/src/app/.miyu/config.toml", "layer": "project", "line": 3}, "problem": "not_tightening", "used": false, "value": false},
        {"origin": {"layer": "default"}, "used": false, "value": "info"},
    ]});
    let said = json!({"applies": "now", "description": "说明。", "name": "名字"});
    let lines = explain("log.level", &item, &said, Language::Chinese, &places());
    assert_eq!(
        lines[1].paint(false),
        "  \"debug\"  环境变量 MIYU_LOG  ← 生效，只管这一次启动\n"
    );
    let project = places().shown("~/src/app/.miyu/config.toml");
    assert_eq!(
        lines[2].paint(false),
        format!("  false    项目配置           {project}:3  ← 不算：比下面几层宽\n")
    );
    let painted = lines[2].paint(true);
    assert!(
        painted.starts_with("\u{1b}[90m") && painted.contains("\u{1b}[31m← 不算"),
        "{painted:?}"
    );
    assert_eq!(lines[3].paint(false), "  \"info\"   默认值\n");
}

#[test]
fn files_are_found_and_shown_with_a_tilde() {
    let places = places();
    assert_eq!(
        places.absolute("system/config.toml"),
        home().join(".miyu").join("system").join("config.toml")
    );
    assert_eq!(
        places.absolute("~/src/app/.miyu/config.toml"),
        home()
            .join("src")
            .join("app")
            .join(".miyu")
            .join("config.toml")
    );
    let elsewhere = std::env::temp_dir().join("elsewhere").join("config.toml");
    assert_eq!(
        places.absolute(&elsewhere.to_string_lossy()),
        elsewhere,
        "绝对路径照原样"
    );
    assert_eq!(
        places.shown("home/admin/settings.toml"),
        format!(
            "~{}",
            ["", ".miyu", "home", "admin", "settings.toml"].join(std::path::MAIN_SEPARATOR_STR)
        )
    );
    assert_eq!(columns("个人设置"), 8);
}

#[test]
fn a_missing_project_config_belongs_at_the_repository_root() {
    let dir = std::env::temp_dir().join(format!("miyu-cli-config-{}", std::process::id()));
    let deep = dir.join("repo").join("src");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::create_dir_all(dir.join("repo").join(".git")).unwrap();
    assert_eq!(
        planned(&deep),
        dir.join("repo").join(".miyu").join("config.toml")
    );
    std::fs::remove_dir_all(dir.join("repo").join(".git")).unwrap();
    assert_eq!(
        planned(&deep),
        deep.join(".miyu").join("config.toml"),
        "没有仓库的是当前目录"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
