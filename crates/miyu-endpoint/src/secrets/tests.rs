//! 密钥的留痕和样本（`docs/designs/samples/journal/secret.changed.jsonl`）一字不差，只有名字；两份密钥文件之间变了的名字：
//! 新设、换掉、删掉，一样的不算。

use std::path::{Path, PathBuf};

use miyu_kernel::id::{AccountId, CommandId};
use miyu_kernel::time::Timestamp;

use super::*;

/// 仓库里的样本。
fn sample(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/journal")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}：{error}", path.display()))
}

/// 用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        Scratch(std::env::temp_dir().join(format!(
            "miyu-endpoint-secrets-{name}-{}",
            std::process::id()
        )))
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

#[test]
fn a_secret_change_is_written_like_the_sample() {
    let scratch = Scratch::new("journal");
    let file = scratch.0.join("journal.jsonl");
    journal::record(
        &file,
        "system/journal.jsonl",
        Timestamp::parse("2026-10-01T08:01:00.000Z").unwrap(),
        By::Person(Person {
            account: AccountId::parse("admin").unwrap(),
        }),
        Some(&CommandId::parse("secret-9f2c4e1a7b3d5f60-1").unwrap()),
        "secret.changed",
        &SecretChanged {
            name: "deepseek",
            action: "set",
            via: "set",
        },
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        sample("secret.changed.jsonl")
    );
}

#[test]
fn changes_between_two_files_are_by_name() {
    let scratch = Scratch::new("changes");
    std::fs::create_dir_all(&scratch.0).unwrap();
    let path = scratch.0.join("secrets.toml");
    let read = |text: &str| {
        std::fs::write(&path, text).unwrap();
        SecretsFile::read(&path, "system/secrets.toml")
    };
    let old = read("kept = \"sk-FAKE-1\"\nswapped = \"sk-FAKE-2\"\ngone = \"sk-FAKE-3\"\n");
    let new = read("kept = \"sk-FAKE-1\"\nswapped = \"sk-FAKE-9\"\nadded = \"sk-FAKE-4\"\n");
    assert_eq!(
        changes(&old, &new),
        [
            ("added".to_string(), "set"),
            ("gone".to_string(), "deleted"),
            ("swapped".to_string(), "replaced"),
        ]
    );
    let shown = format!("{new:?}");
    assert!(!shown.contains("sk-FAKE"), "{shown}");
    let broken = read("kept = \"sk-FAKE-1\n").keeping(&new);
    assert!(broken.last_good);
    assert!(changes(&new, &broken).is_empty(), "读不进来的照上一次的");
}
