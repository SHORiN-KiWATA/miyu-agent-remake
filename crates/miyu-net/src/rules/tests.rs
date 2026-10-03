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
            site: 60,
            author: 60
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

fn url(text: &str) -> reqwest::Url {
    reqwest::Url::parse(text).expect(text)
}

#[test]
fn the_shipped_sites_and_challenge_carry_the_numbers_of_the_blueprint() {
    // net.md「怎么走」第 12、13 条（W-7 再补）。
    let rules = load(&shipped()).unwrap();
    assert_eq!(rules.api.timeout, Duration::from_secs(12));
    assert_eq!(rules.api.accept, "application/json");
    let bilibili = &rules.sites.bilibili;
    assert_eq!(bilibili.site, "哔哩哔哩");
    assert_eq!(bilibili.gone_titles, ["视频去哪了呢？_哔哩哔哩_bilibili"]);
    assert_eq!(rules.sites.mediawiki.thumbnail, 640);
    assert_eq!(rules.challenge.statuses, [403, 503]);
    assert_eq!(
        rules.challenge.headers,
        [("cf-mitigated".to_string(), "challenge".to_string())]
    );
    assert!(
        rules
            .challenge
            .titles
            .iter()
            .any(|title| title == "安全检查")
    );
}

#[test]
fn a_host_matches_itself_and_its_subdomains_only() {
    let rules = load(&shipped()).unwrap();
    let bilibili = &rules.sites.bilibili.hosts;
    for yes in [
        "https://bilibili.com/video/x",
        "https://www.bilibili.com/video/x",
        "https://M.BiliBili.com./video/x",
    ] {
        assert!(bilibili.matches(&url(yes)), "{yes}");
    }
    for no in [
        "https://notbilibili.com/",
        "https://bilibili.com.evil.example/",
        "https://b23.tv/x",
    ] {
        assert!(!bilibili.matches(&url(no)), "{no}");
    }
    assert!(
        rules
            .sites
            .bilibili
            .short_hosts
            .matches(&url("https://b23.tv/x"))
    );
    assert!(rules.sites.youtube.matches(&url("https://youtu.be/x")));
    assert!(
        rules
            .sites
            .youtube
            .matches(&url("https://m.youtube.com/watch?v=x"))
    );
    let wiki = &rules.sites.mediawiki;
    assert_eq!(
        wiki.api_path(&url("https://zh.wikipedia.org/wiki/Rust")),
        Some("/w/api.php")
    );
    assert_eq!(
        wiki.api_path(&url("https://wiki.archlinux.org/title/Pacman")),
        Some("/api.php")
    );
    assert_eq!(wiki.api_path(&url("https://wiki.example.org/")), None);
}

#[test]
fn bad_sites_and_challenge_sections_are_refused() {
    for (from, to, why) in [
        (
            "\"thumbnail\": 640",
            "\"thumbnail\": 640, \"note\": 1",
            "多一格",
        ),
        (
            "\"statuses\": [403, 503]",
            "\"statuses\": [403, 70000]",
            "状态码读不成",
        ),
        ("\"short_hosts\": [\"b23.tv\"],", "", "少一格"),
        (
            "\"site\": \"哔哩哔哩\",",
            "\"site\": \"哔哩哔哩\", \"api\": \"https://api.bilibili.com/\",",
            "B 站不再有接口那一格",
        ),
        ("\"thumbnail\": 640", "\"thumbnail\": 0", "缩略图是 0"),
        (
            "\"max_bytes\": 2097152,\n    \"accept\": \"application/json\"",
            "\"max_bytes\": 0,\n    \"accept\": \"application/json\"",
            "api.max_bytes 是 0",
        ),
    ] {
        let changed = text().replacen(from, to, 1);
        assert_ne!(changed, text(), "{from} 在出厂的那一份里");
        assert!(load(&Scratch::with(&changed).0).is_err(), "{why}");
    }
}
