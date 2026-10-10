//! 遮 key：密钥的原文、常见的写法；太短的原文、不够长的尾巴、不是前缀开头的不遮；写法写错的说是哪一格。

use super::*;

fn shapes() -> KeyShapes {
    KeyShapes::parse("prefixes = [\"sk-\", \"ghp_\", \"AKIA\"]\nmin_tail = 16\n").expect("读得进")
}

#[test]
fn secrets_and_common_key_shapes_are_redacted() {
    let secrets = vec!["hunter2-is-long".to_string(), "ok".to_string()];
    let text = "key 是 sk-abcdefghijklmnop1234，github 的 ghp_ABCDEFGHIJKLMNOP，密码 hunter2-is-long，回答 ok。";
    assert_eq!(
        redact(text, &secrets, &shapes()),
        "key 是 [REDACTED]，github 的 [REDACTED]，密码 [REDACTED]，回答 ok。"
    );
}

#[test]
fn short_tails_and_other_words_stay() {
    let text = "sk-short、task-abcdefghijklmnopqrstu、AKIA1234567890ABCDEF、tasks";
    assert_eq!(
        redact(text, &[], &shapes()),
        "sk-short、task-abcdefghijklmnopqrstu、[REDACTED]、tasks",
        "尾巴不够长的、不是前缀开头的不遮"
    );
    assert_eq!(redact("", &[], &shapes()), "");
    assert_eq!(
        redact("没有 key 的一句话。", &[], &KeyShapes::default()),
        "没有 key 的一句话。"
    );
}

#[test]
fn a_wrong_shape_file_says_where() {
    for (text, says) in [
        ("prefixes = [", "secrets.toml"),
        ("min_tail = 16\n", "prefixes"),
        ("prefixes = [\"\"]\nmin_tail = 16\n", "prefixes"),
        ("prefixes = [\"sk-\"]\n", "min_tail"),
        ("prefixes = [\"sk-\"]\nmin_tail = 0\n", "min_tail"),
        ("prefixes = [\"sk-\"]\nmin_tail = 16\nextra = 1\n", "extra"),
    ] {
        let error = KeyShapes::parse(text).expect_err(text);
        assert!(error.contains(says), "{text:?}：{error}");
    }
}
