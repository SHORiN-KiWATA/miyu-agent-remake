//! 系统日志、账号日志里的一条和样本（`docs/designs/samples/journal/`）一字不差：`body` 的格照图纸的先后；之前没写的不带
//! `old`，删掉的不带 `new`；写不进去的不往上报。

use std::path::{Path, PathBuf};

use miyu_kernel::id::{AccountId, CommandId};
use miyu_kernel::origin::{By, Person};
use miyu_kernel::time::Timestamp;

use super::*;

/// 仓库里的样本。
fn sample(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/journal")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}：{error}", path.display()))
}

/// 用完就删的一份日志。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        Scratch(std::env::temp_dir().join(format!(
            "miyu-endpoint-journal-{name}-{}",
            std::process::id()
        )))
    }

    fn file(&self) -> PathBuf {
        self.0.join("journal.jsonl")
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn admin() -> By {
    By::Person(Person {
        account: AccountId::parse("admin").unwrap(),
    })
}

#[test]
fn a_config_change_is_written_like_the_sample() {
    let scratch = Scratch::new("config");
    let body = ConfigChanged {
        layer: "personal",
        file: "home/admin/settings.toml",
        via: "set",
        changes: vec![KeyChange {
            key: "ui.language".to_string(),
            old: Some(serde_json::json!("en")),
            new: Some(serde_json::json!("zh")),
        }],
    };
    record(
        &scratch.file(),
        "home/admin/journal.jsonl",
        Timestamp::parse("2026-10-01T08:00:00.000Z").unwrap(),
        admin(),
        Some(&CommandId::parse("config-9f2c4e1a7b3d5f60-1").unwrap()),
        "config.changed",
        &body,
    );
    assert_eq!(
        std::fs::read_to_string(scratch.file()).unwrap(),
        sample("config.changed.jsonl")
    );
}

#[test]
fn a_trust_answer_is_written_like_the_sample() {
    let scratch = Scratch::new("trust");
    let body = TrustChanged {
        path: "~/src/app",
        version: "sha256:…",
        trusted: true,
        via: None,
    };
    record(
        &scratch.file(),
        "home/admin/journal.jsonl",
        Timestamp::parse("2026-10-01T08:02:00.000Z").unwrap(),
        admin(),
        Some(&CommandId::parse("config-9f2c4e1a7b3d5f60-2").unwrap()),
        "trust.changed",
        &body,
    );
    assert_eq!(
        std::fs::read_to_string(scratch.file()).unwrap(),
        sample("trust.changed.jsonl")
    );
}

#[test]
fn what_was_not_there_before_or_after_is_left_out() {
    let added = KeyChange {
        key: "ui.language".to_string(),
        old: None,
        new: Some(serde_json::json!("zh")),
    };
    let removed = KeyChange {
        key: "ui.language".to_string(),
        old: Some(serde_json::json!("zh")),
        new: None,
    };
    assert_eq!(
        serde_json::to_string(&added).unwrap(),
        r#"{"key":"ui.language","new":"zh"}"#
    );
    assert_eq!(
        serde_json::to_string(&removed).unwrap(),
        r#"{"key":"ui.language","old":"zh"}"#
    );
}

/// 手改被看到的（施工 8-4）：`by` 是内核，没有 `cause`，`via` 是 `file`；信任的记录多一格 `via`。
#[test]
fn a_hand_edit_is_written_by_the_kernel_without_a_cause() {
    let scratch = Scratch::new("hand");
    let body = ConfigChanged {
        layer: "system",
        file: "system/config.toml",
        via: "file",
        changes: vec![KeyChange {
            key: "log.level".to_string(),
            old: None,
            new: Some(serde_json::json!("debug")),
        }],
    };
    let at = Timestamp::parse("2026-10-01T08:00:00.000Z").unwrap();
    record(
        &scratch.file(),
        "system/journal.jsonl",
        at,
        By::Kernel,
        None,
        "config.changed",
        &body,
    );
    let trust = TrustChanged {
        path: "~/src/app",
        version: "sha256:…",
        trusted: false,
        via: Some("file"),
    };
    record(
        &scratch.file(),
        "system/journal.jsonl",
        at,
        By::Kernel,
        None,
        "trust.changed",
        &trust,
    );
    assert_eq!(
        std::fs::read_to_string(scratch.file()).unwrap(),
        concat!(
            r#"{"seq":1,"at":"2026-10-01T08:00:00.000Z","kind":"config.changed","by":{"kind":"kernel"},"body":{"layer":"system","file":"system/config.toml","via":"file","changes":[{"key":"log.level","new":"debug"}]}}"#,
            "\n",
            r#"{"seq":2,"at":"2026-10-01T08:00:00.000Z","kind":"trust.changed","by":{"kind":"kernel"},"body":{"path":"~/src/app","version":"sha256:…","trusted":false,"via":"file"}}"#,
            "\n",
        )
    );
}
