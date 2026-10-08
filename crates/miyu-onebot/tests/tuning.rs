//! 桥自己的数（施工 O-8，`onebot.md`「施工时定的」第 15 条）：出厂的 `bridge.json` 读得出来，数和图纸写的一样；队列写 0、
//! 多出不认识的格、少了一格、读不了的，读不进来，说是哪个文件。

use std::path::{Path, PathBuf};
use std::time::Duration;

use miyu_onebot::tuning::{FILE, Tuning};

/// 源码树里的资源目录。
fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 一个临时的资源目录，`bridge.json` 写成 `text`：交回目录（用完调的一方删）。
fn with_file(name: &str, text: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("miyu-onebot-tuning-{}-{name}", std::process::id()));
    let file = dir.join(FILE);
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了目录");
    std::fs::write(&file, text).expect("写得进");
    dir
}

#[test]
fn the_shipped_numbers_are_the_blueprints() {
    let tuning = Tuning::load(&resources()).expect("出厂的读得出来");
    assert_eq!(tuning.paths, ["/onebot/v11/ws", "/ws"]);
    assert_eq!(tuning.call_timeout(), Duration::from_secs(10));
    assert_eq!(tuning.write_queue, 64);
    assert_eq!(tuning.inbound_queue, 256);
    assert_eq!(tuning.accept_retry(), Duration::from_millis(100));
    // 跟核心握手最多等 10 秒；`logs -f` 半秒看一次（施工 O-18）。
    assert_eq!(tuning.hello(), Duration::from_secs(10));
    assert_eq!(tuning.follow(), Duration::from_millis(500));
    // 要用场所规则时，隔一秒才看一眼系统的变没变（施工 O-21）。
    assert_eq!(tuning.rules_check(), Duration::from_secs(1));
    // WebUI（施工 O-16）：页面只有三种文件；内容安全策略只许连自己、不许被框起来；验过的登录令牌记 60 秒。
    assert_eq!(
        tuning.web.types.keys().collect::<Vec<_>>(),
        ["css", "html", "js"]
    );
    for must in [
        "default-src 'self'",
        "connect-src 'self'",
        "frame-ancestors 'none'",
    ] {
        assert!(tuning.web.csp.contains(must), "{must}");
    }
    assert_eq!(tuning.web.status_cache(), Duration::from_secs(60));
}

#[test]
fn a_bad_file_is_not_read_and_named() {
    let good = std::fs::read_to_string(resources().join(FILE)).expect("读得了");
    for (name, text) in [
        (
            "write-zero",
            good.replace("\"write_queue\": 64", "\"write_queue\": 0"),
        ),
        (
            "inbound-zero",
            good.replace("\"inbound_queue\": 256", "\"inbound_queue\": 0"),
        ),
        ("unknown", good.replacen('{', "{\"extra\": 1,", 1)),
        (
            "web-unknown",
            good.replace(
                "\"status_cache_seconds\"",
                "\"extra\": 1, \"status_cache_seconds\"",
            ),
        ),
        (
            "web-missing",
            good.replace("\"status_cache_seconds\"", "\"cache\""),
        ),
        (
            "missing",
            good.replace("\"accept_retry_millis\": 100", "\"x\": 1"),
        ),
        (
            // 令牌对不上时重读配置的节流去掉了（施工 O-20）：再写它是多一格。
            "reload-again",
            good.replace(
                "\"hello_seconds\"",
                "\"reload_seconds\": 1,\n  \"hello_seconds\"",
            ),
        ),
        (
            "hello-missing",
            good.replace("\"hello_seconds\": 10", "\"z\": 1"),
        ),
        (
            "follow-missing",
            good.replace("\"follow_millis\": 500", "\"w\": 1"),
        ),
        (
            "rules-check-missing",
            good.replace("\"rules_check_millis\": 1000", "\"v\": 1"),
        ),
        ("not-json", "nope".to_string()),
    ] {
        assert_ne!(text, good, "{name}：真的改了");
        let dir = with_file(name, &text);
        let loaded = Tuning::load(&dir);
        if std::fs::remove_dir_all(&dir).is_err() {
            // 删不掉就留在临时目录里，不影响测试。
        }
        let error = loaded.expect_err(name);
        assert!(error.contains("bridge.json"), "{name}：{error}");
    }
    let error = Tuning::load(Path::new("/nonexistent/miyu-resources")).expect_err("没有文件");
    assert!(error.contains("bridge.json"), "{error}");
}
