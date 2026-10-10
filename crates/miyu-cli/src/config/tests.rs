//! `miyu config` 印的样子（`config.md`「样子」）：`get` 的值、报错一行、`check` 的合计、`explain` 的几行照显示的宽度对齐，
//! 两种语言；核心报的文件换成真的位置、家目录写成 `~/…`；还没有项目配置时它该在哪。

use std::path::PathBuf;

use serde_json::json;

use super::paths::{Places, planned};
use super::render::{columns, explain, problem, toml, values};
use super::{ConfigCommand, ConfigPlan, tidy};
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
        gray: false,
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
        "message":"没有 ui.langauge 这一项。是不是想写 ui.language？已忽略此行。"});
    let file = "~/.miyu/home/admin/settings.toml";
    assert_eq!(
        problem(file, &found, Language::Chinese).paint(false),
        "~/.miyu/home/admin/settings.toml:7:1 警告：没有 ui.langauge 这一项。是不是想写 ui.language？已忽略此行。\n"
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
            "界面语言（ui.language）：终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。立即生效。\n\
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
            "Interface language (ui.language): The language terminals, the web page and the command line use for you. auto follows the terminal or browser. Takes effect immediately.\n\
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

// 施工 8-3：改、写、信任的几个子命令印的字，和图纸「给人看的字」「样子」一字不差（两种语言）；编辑器照 `VISUAL`、`EDITOR`
// 挑，经 shell 跑、文件名里有空格也行。

#[test]
fn set_and_unset_lines_match_the_blueprint() {
    let (zh, en) = (Language::Chinese, Language::English);
    assert_eq!(
        zh.saved("ui.language", "\"zh\"", "personal", "now"),
        "· ui.language = \"zh\" 写进了个人设置，立即生效"
    );
    assert_eq!(
        en.saved("ui.language", "\"zh\"", "personal", "now"),
        "· ui.language = \"zh\" saved to personal settings, takes effect immediately"
    );
    assert_eq!(
        en.saved(
            "permission.start_read_only",
            "true",
            "system",
            "new_session"
        ),
        "· permission.start_read_only = true saved to the system config, applies to new sessions"
    );
    assert_eq!(
        zh.saved("ui.startup", "\"recent\"", "personal", "head_start"),
        "· ui.startup = \"recent\" 写进了个人设置，重启后生效"
    );
    assert_eq!(
        zh.saved_below("ui.language", "\"en\"", "system", "personal", "\"zh\""),
        "· ui.language = \"en\" 写进了系统配置，个人设置里写着 \"zh\"，用的还是 \"zh\""
    );
    assert_eq!(
        en.saved_below("ui.language", "\"en\"", "system", "personal", "\"zh\""),
        "· ui.language = \"en\" saved to the system config, but personal settings say \"zh\", so \"zh\" stays in use"
    );
    assert_eq!(
        en.saved_below("log.level", "\"info\"", "system", "env", "\"debug\""),
        "· log.level = \"info\" saved to the system config, but the environment says \"debug\", so \"debug\" stays in use"
    );
    assert_eq!(zh.already("\"zh\""), "· 本来就是 \"zh\"，没改");
    assert_eq!(en.already("\"zh\""), "· Already \"zh\", nothing changed");
    assert_eq!(
        zh.removed("ui.language", "personal", "\"en\"", "system"),
        "· 从个人设置里删掉了 ui.language，现在是 \"en\"（系统配置）"
    );
    assert_eq!(
        en.removed("ui.language", "personal", "\"en\"", "system"),
        "· Removed ui.language from personal settings. It is now \"en\" (system config)"
    );
    assert_eq!(
        zh.not_there("ui.language", "personal"),
        "· 个人设置里本来就没写 ui.language"
    );
    assert_eq!(
        en.not_there("ui.language", "personal"),
        "· Personal settings did not have ui.language"
    );
    assert_eq!(
        en.not_there("log.level", "system"),
        "· The system config did not have log.level"
    );
}

#[test]
fn edit_and_trust_words_match_the_blueprint() {
    let (zh, en) = (Language::Chinese, Language::English);
    assert_eq!(
        zh.edit_errors(1),
        "有 1 处错误，还没存。回车接着改，输入 q 放弃："
    );
    assert_eq!(
        en.edit_errors(2),
        "2 errors. Nothing saved yet. Press Enter to keep editing, or type q to give up: "
    );
    assert_eq!(zh.edit_saved(&["now"]), "· 存好了，立即生效");
    assert_eq!(
        en.edit_saved(&["now", "new_session"]),
        "· Saved, takes effect immediately, applies to new sessions"
    );
    assert_eq!(en.edit_saved(&[]), "· Saved");
    assert_eq!(
        en.edit_conflict("~/x"),
        "The file changed while you were editing. Nothing saved. Your edit is in ~/x"
    );
    assert_eq!(
        en.editor_failed(Some(3)),
        "The editor did not exit cleanly (3). Nothing saved"
    );
    assert_eq!(zh.editor_failed(None), "编辑器没有正常退出（-），没存");
    assert_eq!(
        en.would_set("~/src/app/.miyu/config.toml"),
        "~/src/app/.miyu/config.toml would set:"
    );
    assert_eq!(
        en.trust_question(),
        "A project config can only make limits stricter. Trust this one? [y/N] "
    );
    assert_eq!(
        en.trust_answered(true),
        "· Trusted. You will be asked again if it changes"
    );
    assert_eq!(
        zh.untrusted_project("~/src/app/.miyu/config.toml"),
        "· 这里的项目配置 ~/src/app/.miyu/config.toml 还没信任，这次没用它：miyu config trust 看一眼再定"
    );
    assert_eq!(
        en.untrusted_project("~/src/app/.miyu/config.toml"),
        "· The project config at ~/src/app/.miyu/config.toml is not trusted yet, so it was not used: run miyu config trust to review it"
    );
    assert_eq!(
        zh.explain_header("启动时打开", "ui.startup", "说明。", "head_start"),
        "启动时打开（ui.startup）：说明。重启后生效。"
    );
}

#[test]
fn the_editor_is_visual_then_editor_then_the_system_one() {
    use super::console::editor;
    assert_eq!(editor(Some("code --wait"), Some("nano")), "code --wait");
    assert_eq!(editor(Some("  "), Some("nano")), "nano", "空的不算");
    assert_eq!(editor(None, Some("nano")), "nano");
    let fallback = match cfg!(windows) {
        true => "notepad",
        false => "vi",
    };
    assert_eq!(editor(None, None), fallback);
    assert_eq!(editor(Some(""), Some("")), fallback);
}

#[test]
fn the_editor_runs_through_the_shell_with_the_file_as_one_argument() {
    use super::console::editor_command;
    let dir = std::env::temp_dir().join(format!("miyu-cli-editor-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("new text.toml");
    std::fs::write(&source, "a = 1\n").unwrap();
    let target = dir.join("my settings.toml");
    std::fs::write(&target, "").unwrap();
    // 一个「编辑器」：把准备好的字抄进交给它的文件。带参数的命令照 shell 的写法。
    let editor = match cfg!(windows) {
        true => format!("copy /y \"{}\"", source.display()),
        false => format!("cp '{}'", source.display()),
    };
    let status = editor_command(&editor, &target).output().unwrap().status;
    assert!(status.success(), "{status:?}");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "a = 1\n");
    std::fs::remove_dir_all(&dir).unwrap();
}

// 施工 8-3 补：生效时机五种都照蓝图说（以前 `next_turn` 说成「当场生效」），认不出的不说；人敲的键照核心的写法规整，
// 带多余引号的也找得到回应。

#[test]
fn every_applies_value_is_said_like_the_blueprint() {
    let (zh, en) = (Language::Chinese, Language::English);
    let key = "providers.dev.models.m-1.window";
    let said = [
        ("now", "立即生效", "takes effect immediately"),
        ("new_session", "新会话生效", "applies to new sessions"),
        ("head_start", "重启后生效", "takes effect after restart"),
        ("next_turn", "下一轮生效", "takes effect next turn"),
        (
            "restart",
            "重启核心后生效",
            "takes effect after the core restarts",
        ),
    ];
    for (applies, chinese, english) in said {
        assert_eq!(
            zh.saved(key, "4096", "personal", applies),
            format!("· {key} = 4096 写进了个人设置，{chinese}")
        );
        assert_eq!(
            en.saved(key, "4096", "personal", applies),
            format!("· {key} = 4096 saved to personal settings, {english}")
        );
        assert_eq!(zh.edit_saved(&[applies]), format!("· 存好了，{chinese}"));
        assert_eq!(
            zh.explain_header("窗口", key, "说明。", applies),
            format!("窗口（{key}）：说明。{chinese}。")
        );
        let mut capital = english.chars();
        let first = capital.next().map(|first| first.to_ascii_uppercase());
        assert_eq!(
            en.explain_header("Window", key, "About.", applies),
            format!(
                "Window ({key}): About. {}{}.",
                first.unwrap_or_default(),
                capital.as_str()
            )
        );
    }
    // 认不出的（核心比命令行新）：不说什么时候生效，不瞎说「当场」。
    assert_eq!(
        zh.saved(key, "4096", "personal", "someday"),
        format!("· {key} = 4096 写进了个人设置")
    );
    assert_eq!(
        en.saved(key, "4096", "personal", "someday"),
        format!("· {key} = 4096 saved to personal settings")
    );
    assert_eq!(zh.edit_saved(&["someday"]), "· 存好了");
    assert_eq!(
        en.edit_saved(&["now", "someday"]),
        "· Saved, takes effect immediately"
    );
    assert_eq!(
        zh.explain_header("窗口", key, "说明。", "someday"),
        format!("窗口（{key}）：说明。")
    );
    assert_eq!(
        en.explain_header("Window", key, "About.", "someday"),
        format!("Window ({key}): About.")
    );
}

#[test]
fn a_typed_key_is_tidied_the_way_the_core_writes_it() {
    // 多余的引号去掉；名字里有点的引号留着；拆不开的原样交，由核心报错。
    assert_eq!(
        tidy("providers.dev.models.\"deepseek-v4-flash\".temperature"),
        "providers.dev.models.deepseek-v4-flash.temperature"
    );
    assert_eq!(
        tidy("providers . dev .models.'m-1'.window"),
        "providers.dev.models.m-1.window"
    );
    assert_eq!(
        tidy("providers.dev.models.\"deepseek-v4.1-flash\".window"),
        "providers.dev.models.\"deepseek-v4.1-flash\".window"
    );
    assert_eq!(tidy("ui.language"), "ui.language");
    assert_eq!(tidy("ui..language"), "ui..language");
    assert_eq!(tidy("ui.\"lang"), "ui.\"lang");
}
