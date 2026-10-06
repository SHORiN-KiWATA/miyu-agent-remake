//! `cargo xtask dev-home`（施工 8-6，`docs/blueprint/models.md`「怎么走」第十条）：照三个环境变量造的数据根，配置照清单读
//! 一处错都没有；已经有配置的不盖、别人的目录不动；真的核心认得出它，照配置连上假服务器，带着 `MIYU_DEV_API_KEY` 的值、
//! 发给写的那个模型，`miyu ask` 答得上来。
//!
//! xtask 不是库：它的 `dev_home.rs` 原样编进这个测试（`#[path]`），测的就是 `cargo xtask dev-home` 用的那一份。

#[path = "../../../xtask/src/dev_home.rs"]
#[allow(dead_code, reason = "命令行那一段这里不用")]
mod dev_home;
mod support;

use std::process::Command;

use miyu_config::Layer;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_ipc::connect_or_start;
use support::{Home, MIYU, within};

use dev_home::{BASE_URL, MODEL, Vars, WINDOW, make};

/// 这棵目录下的每一份文件里都搜不到 `needle`（施工 8-6b：地址不进任何文件）。
fn none_of_the_files_under(dir: &std::path::Path, needle: &str) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                assert!(
                    !bytes
                        .windows(needle.len().max(1))
                        .any(|window| window == needle.as_bytes()),
                    "{} 里搜到了 {needle:?}",
                    path.display()
                );
            }
        }
    }
}

/// 照这几个变量读。
fn read(pairs: &[(&str, &str)]) -> Result<Vars, String> {
    Vars::read(|name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.to_string())
    })
}

#[test]
fn the_variables_are_read_and_checked() {
    let vars = read(&[
        (BASE_URL, " https://relay.example.invalid/v1 "),
        (MODEL, "deepseek-v4.1-flash"),
        (WINDOW, "60000"),
    ])
    .expect("读得出");
    assert_eq!(
        vars,
        Vars {
            base_url: "https://relay.example.invalid/v1".to_string(),
            model: "deepseek-v4.1-flash".to_string(),
            window: Some(60_000),
        }
    );
    let base = (BASE_URL, "https://a.invalid");
    let model = (MODEL, "m");
    assert_eq!(
        read(&[base, model, (WINDOW, " ")]).map(|vars| vars.window),
        Ok(None),
        "空白当没设"
    );
    for bad in [
        vec![model],
        vec![base],
        vec![(BASE_URL, "a.invalid"), model],
        vec![base, (MODEL, "a\nb")],
        vec![base, model, (WINDOW, "0")],
        vec![base, model, (WINDOW, "8k")],
    ] {
        assert!(read(&bad).is_err(), "{bad:?}");
    }
}

#[test]
fn the_config_reads_without_a_single_problem() {
    let vars = read(&[
        (BASE_URL, "https://relay.example.invalid/v1"),
        (MODEL, "deepseek-v4.1-flash"),
        (WINDOW, "60000"),
    ])
    .expect("读得出");
    let text = vars.config();
    let parsed = miyu_config::parse::parse(&miyu_core::settings::items(), Layer::System, &text)
        .expect("TOML 写法对");
    assert_eq!(parsed.problems, Vec::new(), "{text}");
    let value = |key: &str| parsed.entries.get(key).map(|entry| entry.value.toml());
    assert_eq!(
        value("models.chat").as_deref(),
        Some(r#""dev/deepseek-v4.1-flash""#)
    );
    assert_eq!(
        value("providers.dev.driver").as_deref(),
        Some(r#""openai-chat""#)
    );
    assert_eq!(
        value("providers.dev.catalog").as_deref(),
        Some(r#""deepseek""#)
    );
    assert_eq!(
        value("providers.dev.base_url").as_deref(),
        Some(r#"{ env = "MIYU_DEV_BASE_URL" }"#),
        "地址是引用，不是地址本身（施工 8-6b）"
    );
    assert_eq!(
        value("providers.dev.keys").as_deref(),
        Some(r#"[{ env = "MIYU_DEV_API_KEY" }]"#)
    );
    assert_eq!(
        value(r#"providers.dev.models."deepseek-v4.1-flash".window"#).as_deref(),
        Some("60000")
    );
    assert!(
        !text.contains("relay.example.invalid"),
        "地址不进配置文件：{text}"
    );
    let without = Vars {
        window: None,
        ..vars
    }
    .config();
    assert!(!without.contains("window"), "{without}");
}

#[test]
fn an_existing_config_or_a_foreign_directory_is_left_alone() {
    let home = Home::new();
    let vars = read(&[(BASE_URL, "https://a.invalid"), (MODEL, "m")]).expect("读得出");
    let path = make(home.root.path(), &vars).expect("造得出");
    assert_eq!(path, home.root.system().join("config.toml"));
    let error = make(home.root.path(), &vars).expect_err("不盖");
    assert!(error.contains("已经有了"), "{error}");
    let foreign = home.dir.join("foreign");
    std::fs::create_dir_all(&foreign).expect("建得了");
    std::fs::write(foreign.join("notes.txt"), "mine").expect("写得进");
    assert!(
        make(&foreign, &vars).is_err(),
        "认不出是 Miyu 的数据根的不动"
    );
    assert!(!foreign.join("system").exists());
}

#[tokio::test]
async fn a_real_core_on_a_dev_home_answers_through_the_config() {
    let sample = std::fs::read(
        support::resources()
            .join("../docs/designs/samples/drivers/openai-chat/streams/openai-text.sse"),
    )
    .expect("样本读得到");
    let server = Server::start(
        (0..3)
            .map(|_| Reply::stream(vec![Piece::Bytes(sample.clone())]))
            .collect(),
    )
    .await;
    let home = Home::new();
    let vars = read(&[
        (BASE_URL, &server.base_url),
        (MODEL, "deepseek-flash"),
        (WINDOW, "60000"),
    ])
    .expect("读得出");
    make(home.root.path(), &vars).expect("造得出");
    let (held, _) = within(
        "拉起",
        connect_or_start(&home.root, || {
            let mut core = home.core();
            core.env("MIYU_DEV_BASE_URL", &server.base_url)
                .env("MIYU_DEV_API_KEY", "sk-dev-home-test")
                .env("NO_PROXY", "127.0.0.1")
                .env_remove("HTTP_PROXY")
                .env_remove("HTTPS_PROXY")
                .env_remove("ALL_PROXY");
            core
        }),
    )
    .await
    .expect("拉得起");
    let root = home.root.path().to_path_buf();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(MIYU)
            .args(["ask", "在吗"])
            .env("MIYU_HOME", &root)
            .envs(support::offline(&root))
            .env("LANG", "C")
            .env_remove("LC_ALL")
            .env_remove("LC_MESSAGES")
            .output()
            .expect("跑得起来")
    })
    .await
    .expect("没 panic");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{output:?}\n{}",
        home.core_log()
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("你好！"),
        "{output:?}"
    );
    let received = server.received();
    assert_eq!(received[0].path, "/v1/chat/completions");
    assert_eq!(
        received[0].header("authorization"),
        Some("Bearer sk-dev-home-test"),
        "key 照配置里的 {{ env }} 取"
    );
    let body = String::from_utf8_lossy(&received[0].body);
    assert!(body.starts_with(r#"{"model":"deepseek-flash","#), "{body}");
    drop(held);
    home.until_stopped().await;
    // 地址只在环境变量里，数据根的任何一份文件（配置、生成的 Schema、运行日志）里都搜不到（施工 8-6b）。
    none_of_the_files_under(home.root.path(), &server.base_url);
}
