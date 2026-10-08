//! 核心起来时写配置的两份 JSON Schema 和参考文件（`docs/blueprint/config.md`「怎么走」第一条，施工 8-1），真的
//! `miyu core`：照系统的语言写进 `state/config/`，和样本逐字节一样；一样的不重写（修改时间不变），改坏了的写回来；
//! 写不成的记 `WARN`，照样起来。

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use crate::support::{Home, count, hello, within};
use miyu_ipc::connect_or_start;

/// 生成的三份，和它们中文、英文的样本。
const FILES: [(&str, &str); 3] = [
    ("config.schema.json", "config.schema.{}.json"),
    ("settings.schema.json", "settings.schema.{}.json"),
    ("reference.toml", "reference.{}.toml"),
];

/// 样本：`docs/designs/samples/config/` 下这种语言的那一份。
fn sample(pattern: &str, language: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/config")
        .join(pattern.replace("{}", language));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}：{error}", path.display()))
}

/// 拉起核心的命令：系统的语言照 `lang`，别的语言变量都不设。
fn core(home: &Home, lang: &str) -> Command {
    let mut command = home.core();
    command
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", lang);
    command
}

/// 拉起一次核心，握个手，等它空闲了自己走。
async fn run_once(home: &Home, lang: &str) {
    let (connection, token) = within("拉起", connect_or_start(&home.root, || core(home, lang)))
        .await
        .expect("拉得起");
    let reply = hello(connection, &token).await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_stopped().await;
}

fn generated(home: &Home, name: &str) -> PathBuf {
    home.root.state().join("config").join(name)
}

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|error| panic!("{}：{error}", path.display()))
}

#[tokio::test]
async fn the_core_writes_the_files_in_the_system_language() {
    for (lang, language) in [("zh_CN.UTF-8", "zh"), ("en_US.UTF-8", "en")] {
        let home = Home::new();
        run_once(&home, lang).await;
        for (name, pattern) in FILES {
            let written = std::fs::read_to_string(generated(&home, name))
                .unwrap_or_else(|error| panic!("{language} {name} 写了：{error}"));
            assert_eq!(written, sample(pattern, language), "{language} {name}");
        }
        let log = home.core_log();
        assert_eq!(count(&log, "config schema not written"), 0, "{log}");
    }
}

#[tokio::test]
async fn the_same_files_are_left_alone_and_changed_ones_are_put_back() {
    let home = Home::new();
    run_once(&home, "zh_CN.UTF-8").await;
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(86_400);
    for (name, _) in FILES {
        File::options()
            .write(true)
            .open(generated(&home, name))
            .and_then(|file| file.set_modified(old))
            .expect("改得了修改时间");
    }
    std::fs::write(generated(&home, "reference.toml"), "# 手改过\n").expect("写得进");
    run_once(&home, "zh_CN.UTF-8").await;
    for (name, pattern) in FILES {
        let path = generated(&home, name);
        assert_eq!(
            std::fs::read_to_string(&path).expect("在"),
            sample(pattern, "zh"),
            "{name}"
        );
        if name != "reference.toml" {
            assert_eq!(modified(&path), old, "{name} 一样，没重写");
        }
    }
    assert_ne!(
        modified(&generated(&home, "reference.toml")),
        old,
        "改过的写回来了"
    );
}

#[tokio::test]
async fn files_that_cannot_be_written_are_logged_and_the_core_still_starts() {
    let home = Home::new();
    // 该是目录的地方是个文件：三份都写不成。
    let blocked = home.root.state().join("config");
    std::fs::write(&blocked, "not a directory").expect("写得进");
    run_once(&home, "en_US.UTF-8").await;
    let log = home.core_log();
    for (name, _) in FILES {
        let line = log
            .lines()
            .find(|line| line.contains(&format!("file=state/config/{name}")))
            .unwrap_or_else(|| panic!("{name} 记了一条：{log}"));
        assert!(
            line.contains(" WARN  config   config schema not written "),
            "{line}"
        );
        assert!(line.contains(" error="), "{line}");
    }
    assert_eq!(count(&log, "starting"), 1, "{log}");
    assert_eq!(
        std::fs::read_to_string(&blocked).expect("在"),
        "not a directory",
        "没动它"
    );
}
