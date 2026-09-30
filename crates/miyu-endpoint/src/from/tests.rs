//! 收别的 harness 报的名字（施工 7-10）：去掉控制字符、截到 128 字节不截断一个字，空的参数不对。

use super::*;

/// 收下的名字；参数不对的是 `None`。
fn kept(name: &str) -> Option<String> {
    match harness(name) {
        Ok(By::Harness(harness)) => Some(harness.name.as_str().to_string()),
        Ok(other) => panic!("只会是 harness：{other:?}"),
        Err(_) => None,
    }
}

#[test]
fn a_plain_name_is_kept_as_it_is() {
    assert_eq!(kept("claude-code").as_deref(), Some("claude-code"));
    assert_eq!(
        kept(" 我的 脚本 ").as_deref(),
        Some(" 我的 脚本 "),
        "空格不是控制字符"
    );
}

#[test]
fn control_characters_go_first() {
    assert_eq!(
        kept("\u{1b}[1mclaude\t-code\r\n\u{7f}\u{85}").as_deref(),
        Some("[1mclaude-code")
    );
    // 去掉控制字符以后再量长短：128 个字母夹着控制字符，照样都在。
    let name = "a\u{7}".repeat(128);
    assert_eq!(kept(&name), Some("a".repeat(128)));
}

#[test]
fn it_is_cut_to_128_bytes_without_splitting_a_character() {
    assert_eq!(kept(&"a".repeat(129)), Some("a".repeat(128)));
    assert_eq!(
        kept(&"界".repeat(43)),
        Some("界".repeat(42)),
        "126 字节，第 43 个放不下"
    );
    let mixed = format!("{}界", "a".repeat(126));
    assert_eq!(kept(&mixed), Some("a".repeat(126)), "差一个字节也不截断");
}

#[test]
fn nothing_left_is_bad() {
    assert_eq!(kept(""), None);
    assert_eq!(kept("\n\t\u{0}"), None);
}
