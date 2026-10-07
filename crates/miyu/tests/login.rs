//! 真核心上的 `miyu login`、`miyu logout`（施工 8-5，`docs/blueprint/config.md`「守着它的」）：管道进来的 key 存进
//! `system/secrets.toml`，`--list` 两种格式里没有 key，`logout` 删掉、没设过的退出码 1；名字写错、不写名字又不在终端里、
//! `--format` 不带 `--list` 退出码 2；核心没在跑的拉起来（施工 8-6）；帮助页两种语言。整份运行日志（`trace`）、
//! 系统日志、屏幕上都搜不到 key。

mod support;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;
use support::{Home, MIYU, count, within};

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";
const FAKE_2: &str = "sk-FAKE-KEY-FOR-TESTS-0002";

/// 在数据根 `root` 上跑 `miyu <args>`，标准输入是 `input`（管道），界面语言是 `lang`，没有 key。
fn miyu(root: &Path, lang: &str, args: &[&str], input: &str) -> Output {
    let mut child = Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", root)
        .envs(support::offline(root))
        .env("MIYU_RESOURCES", support::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR")
        .env_remove("NO_COLOR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("跑得起来");
    let mut stdin = child.stdin.take().expect("有标准输入");
    // 不读标准输入就退出的（参数不对），写不进去也不要紧。
    let _written = stdin.write_all(input.as_bytes());
    drop(stdin);
    child.wait_with_output().expect("等得到")
}

/// 在阻塞线程里跑：核心在这个测试的运行时里。屏幕上没有 key。
async fn run(root: &Path, lang: &str, args: &[&str], input: &str) -> Output {
    let (root, lang, input) = (root.to_path_buf(), lang.to_string(), input.to_string());
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    let output = tokio::task::spawn_blocking(move || {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        miyu(&root, &lang, &args, &input)
    })
    .await
    .expect("没 panic");
    let screen = format!("{}{}", text(&output.stdout), text(&output.stderr));
    assert!(!screen.contains("FAKE"), "屏幕上有 key：{screen}");
    output
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// 退出码、标准输出、标准错误。
fn seen(output: &Output) -> (Option<i32>, String, String) {
    (
        output.status.code(),
        text(&output.stdout),
        text(&output.stderr),
    )
}

#[tokio::test]
async fn keys_go_in_by_pipe_list_without_values_and_come_out() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let core = || {
        let mut core = home.core();
        core.env("MIYU_LOG", "trace");
        core
    };
    let (held, _) = within("拉起", connect_or_start(&home.root, core))
        .await
        .expect("拉得起");

    let saved = run(&root, "C", &["login", "deepseek"], &format!("{FAKE}\n")).await;
    assert_eq!(
        seen(&saved),
        (
            Some(0),
            String::new(),
            "· Saved the key for deepseek\n".to_string()
        )
    );
    let replaced = run(&root, "zh_CN.UTF-8", &["login", "deepseek"], FAKE_2).await;
    assert_eq!(
        seen(&replaced),
        (
            Some(0),
            String::new(),
            "· 换掉了 deepseek 的 key\n".to_string()
        )
    );
    let secrets = root.join("system").join("secrets.toml");
    let written = std::fs::read_to_string(&secrets).expect("写了");
    assert!(
        written.ends_with(&format!("deepseek = \"{FAKE_2}\"\n")),
        "{written}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&secrets).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let empty = run(&root, "C", &["login", "other"], "  \n").await;
    assert_eq!(
        seen(&empty),
        (Some(1), String::new(), "No key was given\n".to_string())
    );

    let listed = run(&root, "C", &["login", "--list"], "").await;
    assert_eq!(
        seen(&listed),
        (Some(0), "deepseek  set\n".to_string(), String::new())
    );
    let json = run(&root, "C", &["login", "--list", "--format", "json"], "").await;
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).expect("是 JSON");
    assert_eq!(
        json,
        serde_json::json!({"secrets": [{"name": "deepseek", "set": true, "used_by": []}]})
    );

    let out = run(&root, "C", &["logout", "deepseek"], "").await;
    assert_eq!(
        seen(&out),
        (
            Some(0),
            String::new(),
            "· Deleted the key for deepseek\n".to_string()
        )
    );
    let again = run(&root, "zh_CN.UTF-8", &["logout", "deepseek"], "").await;
    assert_eq!(
        seen(&again),
        (
            Some(1),
            String::new(),
            "deepseek 没有设过 key\n".to_string()
        )
    );
    let none = run(&root, "C", &["login", "--list"], "").await;
    assert_eq!(text(&none.stdout), "No keys yet\n");

    drop(held);
    home.until_stopped().await;
    let log = home.core_log();
    assert_eq!(
        count(&log, "secret changed name=deepseek action=set via=set"),
        1,
        "{log}"
    );
    assert_eq!(
        count(&log, "secret changed name=deepseek action=replaced via=set"),
        1,
        "{log}"
    );
    assert_eq!(
        count(&log, "secret changed name=deepseek action=deleted via=set"),
        1,
        "{log}"
    );
    assert!(
        count(&log, "TRACE") + count(&log, "DEBUG") > 0,
        "照 trace 记：{log}"
    );
    assert!(!log.contains("FAKE"), "运行日志里有 key：{log}");
    let journal = std::fs::read_to_string(root.join("system").join("journal.jsonl")).expect("记了");
    assert_eq!(journal.lines().count(), 3, "{journal}");
    assert!(!journal.contains("FAKE"), "{journal}");
}

#[tokio::test]
async fn misuse_and_no_core_are_refused_before_reading_a_key() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let bad = run(&root, "C", &["login", "Deep Seek"], FAKE).await;
    assert_eq!(bad.status.code(), Some(2));
    assert!(
        text(&bad.stderr).starts_with("Deep Seek cannot name a key"),
        "{bad:?}"
    );
    let unpicked = run(&root, "zh_CN.UTF-8", &["login"], FAKE).await;
    assert_eq!(
        seen(&unpicked),
        (
            Some(2),
            String::new(),
            "要在终端里选，或者写 miyu login <名字>\n".to_string()
        )
    );
    let format = run(&root, "C", &["login", "--format", "json"], "").await;
    assert_eq!(format.status.code(), Some(2), "--format 只给 --list");
    let both = run(&root, "C", &["login", "deepseek", "--list"], "").await;
    assert_eq!(both.status.code(), Some(2));
    // 核心没在跑的拉起来（施工 8-6 起一律拉起，原来没设 DEEPSEEK_API_KEY 的退出码 5）。
    let mut command = Command::new(MIYU);
    command
        .args(["login", "deepseek"])
        .env("MIYU_HOME", &root)
        .envs(support::offline(&root))
        .env("MIYU_RESOURCES", support::resources())
        .env("LANG", "C")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR");
    let dir = home.dir.clone();
    let started = tokio::task::spawn_blocking(move || support::run_starting(&dir, command, FAKE))
        .await
        .expect("没 panic");
    assert_eq!(started.status.code(), Some(0), "{started:?}");
    assert!(root.join("system").join("secrets.toml").exists());
    home.kill_core().await;
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        let help = run(&root, lang, &["login", "-h"], "").await;
        assert_eq!(text(&help.stdout), page(language, Page::Login));
        let help = run(&root, lang, &["logout", "--help"], "").await;
        assert_eq!(text(&help.stdout), page(language, Page::Logout));
    }
}

#[tokio::test]
async fn config_check_reports_a_bad_secrets_file_without_its_values() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let secrets = root.join("system").join("secrets.toml");
    std::fs::write(
        &secrets,
        format!("ok = \"{FAKE}\"\nDeepSeek = \"{FAKE_2}\"\n"),
    )
    .expect("写得进");
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let checked = run(&root, "C", &["check"], "").await;
    let (code, out, _) = seen(&checked);
    assert_eq!(code, Some(1), "{checked:?}");
    assert!(
        out.contains("secrets.toml:2:1 error: DeepSeek is not a valid secret name"),
        "{out}"
    );
    assert!(out.ends_with("1 error\n"), "{out}");
    // 只查密钥文件的：照核心手里的问题，只有它（施工 8-30）。
    let secrets_only = run(&root, "C", &["check", &secrets.to_string_lossy()], "").await;
    let (_, out, _) = seen(&secrets_only);
    assert!(out.contains("secrets.toml:2:1 error"), "{out}");
    assert!(out.ends_with("1 error\n"), "{out}");
    drop(held);
}
