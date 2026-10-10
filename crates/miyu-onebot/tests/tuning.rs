//! 桥自己的数（施工 O-8，`onebot.md`「施工时定的」第 15 条）：出厂的 `bridge.json` 读得出来，数和图纸写的一样；队列写 0、
//! 判官的并发写 0（施工 O-23 下）、排着的过期写 0（施工 O-25 中）、多出不认识的格（施工 O-28 下去掉的 `web` 也是）、少了一格
//! （施工 O-25 下的贴表情两格也是）、读不了的，读不进来，说是哪个文件。

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
    // 群成员的名字记十分钟（施工 O-22）。
    assert_eq!(tuning.member_names(), Duration::from_secs(600));
    // 判官全局最多同时问 4 个，排队最多等 15 秒（施工 O-23 下，18 第七节）。
    assert_eq!(tuning.judge_concurrency, 4);
    assert_eq!(tuning.judge_queue(), Duration::from_secs(15));
    // 判官带的人格原文记一分钟（施工 O-23 补）。
    assert_eq!(tuning.judge_persona(), Duration::from_secs(60));
    // 群里的命令回执发出去 3 秒后撤回（施工 O-25 上，18 第十节）。
    assert_eq!(tuning.receipt_recall(), Duration::from_secs(3));
    // 出站排着的一分钟过期（施工 O-25 中）。
    assert_eq!(tuning.queue_expire(), Duration::from_secs(60));
    // 群里判过要回的那一条贴 QQ 表情 289，十分钟还没回的摘掉（施工 O-25 下，18 第七节）。
    assert_eq!(tuning.reaction_emoji, "289");
    assert_eq!(tuning.reaction(), Duration::from_secs(600));
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
            // 桥自己的网页的数随网页去掉了（施工 O-28 下）：再写它是多一格。
            "web-again",
            good.replacen('{', "{\"web\": {\"status_cache_seconds\": 60},", 1),
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
        (
            "member-names-missing",
            good.replace("\"member_names_seconds\": 600", "\"u\": 1"),
        ),
        (
            "judges-zero",
            good.replace("\"judge_concurrency\": 4", "\"judge_concurrency\": 0"),
        ),
        (
            "judge-queue-missing",
            good.replace("\"judge_queue_seconds\": 15", "\"t\": 1"),
        ),
        (
            "judge-persona-missing",
            good.replace("\"judge_persona_seconds\": 60", "\"s\": 1"),
        ),
        (
            "expire-missing",
            good.replace("\"queue_expire_seconds\": 60", "\"r\": 1"),
        ),
        (
            "expire-zero",
            good.replace(
                "\"queue_expire_seconds\": 60",
                "\"queue_expire_seconds\": 0",
            ),
        ),
        (
            "emoji-missing",
            good.replace("\"reaction_emoji\": \"289\"", "\"q\": 1"),
        ),
        (
            "emoji-not-text",
            good.replace("\"reaction_emoji\": \"289\"", "\"reaction_emoji\": 289"),
        ),
        (
            "reaction-missing",
            good.replace("\"reaction_seconds\": 600", "\"p\": 1"),
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
