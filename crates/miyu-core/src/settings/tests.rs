//! 起来时生成的三份：字照系统的语言挑；资源里没有配置的字的、目录建不了的，每一份记一条 `WARN`，不出错。照样本逐字节
//! 比、真核心走一遍见 `tests/settings.rs`、`crates/miyu/tests/settings.rs`。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_log::{LevelFilter, Memory};
use miyu_store::env::{Env, Platform};

use super::*;

/// 一个用完就删的临时目录：数据根在 `root/`，资源目录在 `resources/`。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-core-settings-{}-{n}", std::process::id())))
    }

    fn root(&self) -> DataRoot {
        let root = DataRoot::locate(&Env {
            platform: Platform::current(),
            miyu_home: Some(self.0.join("root").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        })
        .expect("MIYU_HOME 是绝对路径");
        root.prepare().expect("建得了骨架");
        root
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

/// 源码树的资源目录。
fn resources() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 生成一次，交回记下的行。
fn generated(root: &DataRoot, resources: &ResourceRoot, locale: Option<&str>) -> Vec<String> {
    let memory = Memory::new();
    tracing::subscriber::with_default(
        miyu_log::subscriber(memory.clone(), LevelFilter::INFO, None),
        || generate(root, resources, locale, &Values::defaults(&items())),
    );
    memory.lines()
}

#[test]
fn the_files_follow_the_system_language() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let dir = root.state().join("config");
    assert_eq!(
        generated(&root, &resources(), Some("ja_JP.UTF-8")),
        Vec::<String>::new()
    );
    let reference = std::fs::read_to_string(dir.join("reference.toml")).expect("写了");
    assert!(reference.contains("実行ログのレベル"), "{reference}");
    assert_eq!(generated(&root, &resources(), None), Vec::<String>::new());
    let reference = std::fs::read_to_string(dir.join("reference.toml")).expect("写了");
    assert!(
        reference.contains("Runtime log level"),
        "没有系统语言的照英文：{reference}"
    );
    for name in FILES {
        assert!(dir.join(name).is_file(), "{name}");
    }
}

#[test]
fn missing_words_are_logged_for_each_file_and_nothing_is_written() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let bare = scratch.0.join("resources");
    std::fs::create_dir_all(bare.join("core/human")).expect("建得了目录");
    std::fs::write(bare.join("core/human/en.json"), r#"{"said":{}}"#).expect("写得进");
    let lines = generated(&root, &ResourceRoot::at(&bare), None);
    assert_eq!(lines.len(), 3, "{lines:?}");
    for (line, name) in lines.iter().zip(FILES) {
        assert!(
            line.contains(" WARN  config   config schema not written "),
            "{line}"
        );
        assert!(
            line.contains(&format!("file=state/config/{name}")),
            "{line}"
        );
        assert!(line.contains("error=\"no words for "), "{line}");
    }
    assert!(!root.state().join("config").exists(), "什么都没写");
    // 读不懂的字：也是每一份一条，原因是哪一份读不懂。
    std::fs::write(bare.join("core/human/en.json"), "{").expect("写得进");
    let lines = generated(&root, &ResourceRoot::at(&bare), None);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(
        lines.iter().all(|line| line.contains("en.json")),
        "{lines:?}"
    );
}
