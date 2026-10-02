use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 源码树里的资源目录。
fn shipped() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 一个临时的资源目录，`link_preview.json` 写成 `text`。
struct Scratch(PathBuf);

impl Scratch {
    fn with(text: &str) -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-net-rules-{}-{n}", std::process::id()));
        let file = file(&dir);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
        Scratch(dir)
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

/// 出厂的那一份。
fn text() -> String {
    std::fs::read_to_string(file(&shipped())).unwrap()
}

#[test]
fn the_shipped_rules_carry_the_numbers_of_the_blueprint() {
    let rules = load(&shipped()).unwrap();
    // net.md「怎么走」第 6、8、9 条，第 4 条（5 跳），第 7 条（截断），第 10 条（客户端）。
    assert_eq!(rules.found, Duration::from_secs(6 * 3600));
    assert_eq!(rules.no_preview, Duration::from_secs(15 * 60));
    assert_eq!(rules.unreachable, Duration::from_secs(45));
    assert_eq!(rules.entries, 512);
    assert_eq!(rules.page.timeout, Duration::from_secs(12));
    assert_eq!(rules.page.max_bytes, 2 * 1024 * 1024);
    assert_eq!(rules.image.timeout, Duration::from_secs(8));
    assert_eq!(rules.image.max_bytes, 3 * 1024 * 1024);
    assert_eq!(rules.redirects, 5);
    assert_eq!(
        rules.clip,
        Clip {
            title: 120,
            description: 300,
            site: 60
        }
    );
    assert_eq!(rules.client_ttl, Duration::from_secs(120));
    assert_eq!(rules.client_keep, 32);
    // 不先要 AVIF：它不在收的五种里（「起草时定的」第 7 条）。
    assert!(
        !rules.image.accept.contains("avif"),
        "{}",
        rules.image.accept
    );
}

#[test]
fn an_unknown_or_missing_field_is_refused() {
    let extra = text().replacen("\"redirects\"", "\"note\": \"x\",\n  \"redirects\"", 1);
    assert!(load(&Scratch::with(&extra).0).is_err(), "多一格不认");
    let missing = text().replacen("\"redirects\": 5,", "", 1);
    assert!(load(&Scratch::with(&missing).0).is_err(), "少一格不认");
    assert!(load(&Scratch::with("not json").0).is_err());
    let nowhere = std::env::temp_dir().join("miyu-net-rules-nowhere");
    assert!(load(&nowhere).is_err(), "没有这一份");
}

#[test]
fn a_zero_is_refused_and_named() {
    for (from, to, name) in [
        (
            "\"found_seconds\": 21600",
            "\"found_seconds\": 0",
            "found_seconds",
        ),
        ("\"entries\": 512", "\"entries\": 0", "entries"),
        (
            "\"max_bytes\": 2097152",
            "\"max_bytes\": 0",
            "page.max_bytes",
        ),
        ("\"title\": 120", "\"title\": 0", "clip.title"),
        ("\"keep\": 32", "\"keep\": 0", "clients.keep"),
    ] {
        let changed = text().replacen(from, to, 1);
        assert_ne!(changed, text(), "{from} 在出厂的那一份里");
        let error = load(&Scratch::with(&changed).0).unwrap_err();
        assert!(error.to_string().contains(name), "{error}");
    }
}
