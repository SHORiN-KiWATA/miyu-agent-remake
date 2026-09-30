//! 真核心带着配置起来（施工 8-2，`docs/blueprint/config.md`「守着它的」）：`log.level` 照系统配置换、运行日志记从哪来、
//! 有问题的文件记一条；`miyu config get`、`explain` 印出的来源对，`check`、`path` 照样子印，握手以后照 `ui.language`
//! 说话；`miyu ask` 起头说配置有错；帮助页跟着界面语言；参数不对退出码 2；没有 key、核心也没在跑的不拉起。

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;
use support::{Home, MIYU, count, within};

/// 系统配置：日志记到 debug，界面英文，开局只读写错了（一处错误）。
const SYSTEM: &str = "[log]\nlevel = \"debug\"\n\n[ui]\nlanguage = \"en\"\n\n[permission]\nstart_read_only = \"yes\"\n";

/// 个人设置：界面中文（第 3 行），一个拼错的键（警告）。
const PERSONAL: &str = "\n[ui]\nlanguage = \"zh\"\nlangauge = \"ja\"\n";

/// 在数据根 `root` 上、工作目录 `cwd` 里跑 `miyu <args>`：没有 key，界面语言是 `lang`。
fn miyu(root: &Path, cwd: &Path, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .current_dir(cwd)
        .env("MIYU_HOME", root)
        .env("MIYU_RESOURCES", support::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("XDG_RUNTIME_DIR")
        .env_remove("NO_COLOR")
        .output()
        .expect("跑得起来")
}

/// 在阻塞线程里跑：核心在这个测试的运行时里。
async fn run(root: &Path, cwd: &Path, lang: &str, args: &[&str]) -> Output {
    let (root, cwd, lang): (PathBuf, PathBuf, String) =
        (root.to_path_buf(), cwd.to_path_buf(), lang.to_string());
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    tokio::task::spawn_blocking(move || {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        miyu(&root, &cwd, &lang, &args)
    })
    .await
    .expect("没 panic")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// 写一份配置文件，目录没有的建上。
fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
    std::fs::write(path, text).expect("写得进");
}

#[tokio::test]
async fn a_core_with_three_layers_says_where_each_value_came_from() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    write(&root.join("system").join("config.toml"), SYSTEM);
    let personal = root.join("home").join("admin").join("settings.toml");
    write(&personal, PERSONAL);
    // 核心那边的系统语言是英文：生成的文件还是照个人设置写的中文。
    let english = || {
        let mut core = home.core();
        core.env("LANG", "C")
            .env_remove("LC_ALL")
            .env_remove("LC_MESSAGES");
        core
    };
    let (held, _) = within("拉起", connect_or_start(&home.root, english))
        .await
        .expect("拉得起");
    let log = home.core_log();
    assert_eq!(count(&log, "log level level=debug from=config"), 1, "{log}");
    assert_eq!(
        count(
            &log,
            "config problems file=system/config.toml errors=1 warnings=0"
        ),
        1,
        "{log}"
    );
    assert_eq!(
        count(&log, "home/admin/settings.toml errors=0 warnings=1"),
        1,
        "{log}"
    );
    let cwd = std::env::temp_dir();

    let got = run(&root, &cwd, "C", &["config", "get", "ui.language"]).await;
    assert_eq!(
        (got.status.code(), stdout(&got)),
        (Some(0), "zh\n".to_string()),
        "{got:?}"
    );
    let all = run(&root, &cwd, "C", &["config", "get"]).await;
    assert_eq!(
        stdout(&all),
        "log.level = \"debug\"\npermission.start_read_only = false\nui.language = \"zh\"\n"
    );
    let json = run(
        &root,
        &cwd,
        "C",
        &["config", "get", "log.level", "--format", "json"],
    )
    .await;
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("是 JSON");
    assert_eq!(
        json["log.level"]["origin"],
        serde_json::json!({"file": "system/config.toml", "layer": "system", "line": 2})
    );

    // 个人设置定了中文：LANG 是英文也说中文。
    let explained = run(&root, &cwd, "C", &["config", "explain", "ui.language"]).await;
    assert_eq!(explained.status.code(), Some(0), "{explained:?}");
    let text = stdout(&explained);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].starts_with("界面语言（ui.language）："), "{text}");
    assert!(
        lines[1].starts_with("  \"zh\"    个人设置  ")
            && lines[1].ends_with("settings.toml:3  ← 生效"),
        "{text}"
    );
    assert!(
        lines[2].starts_with("  \"en\"    系统配置  ") && lines[2].ends_with("config.toml:5"),
        "{text}"
    );
    assert_eq!(lines[3], "  \"auto\"  默认值");

    let unknown = run(&root, &cwd, "C", &["config", "get", "ui.langauge"]).await;
    assert_eq!(unknown.status.code(), Some(1));
    assert_eq!(
        stderr(&unknown),
        "没有 ui.langauge 这一项。是不是想写 ui.language？\n"
    );

    // 日志照系统配置记到 debug：连接的每一条请求都记下了（`log.level` 真的换上了）。
    assert!(
        count(&home.core_log(), "request method=config.get") >= 1,
        "{}",
        home.core_log()
    );
    // 生成的参考文件照管理员的 `ui.language`（个人设置写的中文）。
    let reference =
        std::fs::read_to_string(root.join("state").join("config").join("reference.toml"))
            .expect("生成了");
    assert!(reference.contains("界面语言"), "{reference}");
    let path = run(&root, &cwd, "C", &["config", "path"]).await;
    assert_eq!(PathBuf::from(stdout(&path).trim_end()), personal);
    let system = run(&root, &cwd, "C", &["config", "path", "--system"]).await;
    assert_eq!(
        PathBuf::from(stdout(&system).trim_end()),
        root.join("system").join("config.toml")
    );
    drop(held);
}

#[tokio::test]
async fn check_reports_each_problem_on_a_line_and_the_total() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    write(&root.join("system").join("config.toml"), SYSTEM);
    write(
        &root.join("home").join("admin").join("settings.toml"),
        "ui.language = \"en\"\nx.y = 1\n",
    );
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = std::env::temp_dir();
    let checked = run(&root, &cwd, "zh_CN.UTF-8", &["config", "check"]).await;
    assert_eq!(checked.status.code(), Some(1), "有错误：{checked:?}");
    let text = stdout(&checked);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(
        lines[0].ends_with("config.toml:8:19 error: permission.start_read_only needs true or false, not \"yes\". Write permission.start_read_only = true. Using false (the default) for now."),
        "个人设置定了英文：{text}"
    );
    assert!(
        lines[1].ends_with(
            "settings.toml:2:1 warning: There is no x.y. The line is ignored and kept as it is."
        ),
        "{text}"
    );
    assert_eq!(lines[2], "1 error, 1 warning");

    let file = std::env::temp_dir().join(format!("miyu-check-{}.toml", std::process::id()));
    write(&file, "log.level = \"verbose\"\n");
    let one = run(
        &root,
        &cwd,
        "C",
        &["config", "check", "--system", &file.to_string_lossy()],
    )
    .await;
    assert_eq!(one.status.code(), Some(1));
    assert!(stdout(&one).contains(":1:13 error: log.level must be error, warn, info, debug, trace or off, not \"verbose\"."), "{one:?}");
    let personal = run(
        &root,
        &cwd,
        "C",
        &["config", "check", &file.to_string_lossy()],
    )
    .await;
    assert!(
        stdout(&personal).contains("log.level belongs in the system config."),
        "照个人设置查"
    );
    write(&file, "ui.language = \"zh\"\n");
    let clean = run(
        &root,
        &cwd,
        "C",
        &[
            "config",
            "check",
            "--format",
            "json",
            &file.to_string_lossy(),
        ],
    )
    .await;
    assert_eq!(
        (clean.status.code(), stdout(&clean)),
        (Some(0), "{\"problems\":[]}\n".to_string())
    );
    std::fs::remove_file(&file).expect("删得掉");
    drop(held);
}

#[tokio::test]
async fn ask_first_says_the_config_has_errors_and_path_says_where_a_project_config_goes() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    write(&root.join("system").join("config.toml"), SYSTEM);
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = std::env::temp_dir().join(format!("miyu-config-cwd-{}", std::process::id()));
    std::fs::create_dir_all(cwd.join(".git")).expect("建得了");
    let asked = run(&root, &cwd, "zh_CN.UTF-8", &["ask", "在吗"]).await;
    let first = stderr(&asked)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        first, "· 1 error in the config: run miyu config check to see it",
        "系统配置定了英文：{asked:?}"
    );
    let project = run(&root, &cwd, "C", &["config", "path", "--project"]).await;
    assert_eq!(project.status.code(), Some(0));
    let real = std::fs::canonicalize(&cwd).expect("在");
    let printed = PathBuf::from(stdout(&project).trim_end());
    assert!(
        printed == cwd.join(".miyu").join("config.toml")
            || printed == real.join(".miyu").join("config.toml"),
        "{printed:?}"
    );
    assert_eq!(stderr(&project), "This file does not exist yet\n");
    std::fs::remove_dir_all(&cwd).expect("删得掉");
    drop(held);
}

#[test]
fn the_help_follows_the_language_and_bad_arguments_are_refused() {
    let home = Home::new();
    let cwd = std::env::temp_dir();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [
            &["config", "-h"][..],
            &["config", "get", "--help"],
            &["help", "config"],
        ] {
            let output = miyu(home.root.path(), &cwd, lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                stdout(&output),
                page(language, Page::Config),
                "{lang} {args:?}"
            );
        }
    }
    for args in [
        &["config"][..],
        &["config", "get", "--system"],
        &["config", "path", "--system", "--project"],
        &["config", "explain"],
        &["config", "set", "ui.language", "zh"],
    ] {
        let output = miyu(home.root.path(), &cwd, "C", args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
    }
}

#[test]
fn without_a_key_or_a_running_core_nothing_is_started() {
    let home = Home::new();
    let output = miyu(
        home.root.path(),
        &std::env::temp_dir(),
        "zh_CN.UTF-8",
        &["config", "get"],
    );
    assert_eq!(output.status.code(), Some(5), "{output:?}");
    assert_eq!(
        stderr(&output),
        "没有可用的模型：设环境变量 DEEPSEEK_API_KEY\n"
    );
}
