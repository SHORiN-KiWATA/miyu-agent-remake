//! 违规关键词（`chat.md` 第二条「守着它的」）：大小写、中文、空关键词、主人不查；base64 够长的、不够长的、解出来不可打印的、
//! 读不成 UTF-8 的、只看前几个字符、关键词在 `max_chars` 之后的、带 `=` 的、长度不是 4 的倍数的、被别的字截开的。

use crate::VenueKind;

use super::super::test_support::{at, ctx, judge, msg};
use super::super::{Base64, Ctx, Flag, Outcome, Standing};

/// 违规关键词是 `keywords`、base64 的三个数是 `base64`，群里别的人发 `text`：插旗没有。旗只插一面，也不拦。
fn flagged_with(keywords: &[&str], base64: Base64, standing: Standing, text: &str) -> bool {
    let mut watch: Ctx = ctx();
    watch.moderation.keywords = keywords.iter().map(|word| word.to_string()).collect();
    watch.moderation.base64 = base64;
    let mut sent = msg(standing, VenueKind::Group);
    sent.text = text.to_string();
    let verdict = judge(&sent, &watch, at(12, 0));
    assert_eq!(verdict.outcome, Outcome::Pass, "{text}");
    match verdict.flags.as_slice() {
        [] => false,
        [Flag::Moderation] => true,
        more => panic!("{more:?}"),
    }
}

/// 测试随手定的 base64 三个数：至少 8 个字符、看前 100 个、可打印的至少九成。
fn usual() -> Base64 {
    Base64 {
        min_chars: 8,
        max_chars: 100,
        printable: 900,
    }
}

/// 违规关键词只有 `spam`，群里别的人发 `text`。
fn flagged(text: &str) -> bool {
    flagged_with(&["spam"], usual(), Standing::Member, text)
}

#[test]
fn keyword_is_a_substring_ignoring_ascii_case() {
    assert!(flagged("spam"));
    assert!(flagged("buy SPAM now"));
    assert!(flagged("xSpAmx"));
    assert!(!flagged("spa m"));
    assert!(!flagged("hello"));
    assert!(flagged_with(
        &["SpAm"],
        usual(),
        Standing::Member,
        "a spam b"
    ));
}

#[test]
fn other_chars_compare_as_they_are() {
    let words = ["违规"];
    assert!(flagged_with(
        &words,
        usual(),
        Standing::Member,
        "这是违规内容"
    ));
    assert!(!flagged_with(
        &words,
        usual(),
        Standing::Member,
        "这是违 规内容"
    ));
    // 不是 ASCII 的字母不折大小写。
    assert!(!flagged_with(&["ä"], usual(), Standing::Member, "Ä"));
    assert!(flagged_with(&["ä"], usual(), Standing::Member, "bär"));
    // 全角的不是 ASCII。
    assert!(!flagged_with(
        &["spam"],
        usual(),
        Standing::Member,
        "ｓｐａｍ"
    ));
}

#[test]
fn any_keyword_hits_and_empty_ones_do_not_count() {
    assert!(flagged_with(
        &["ham", "spam"],
        usual(),
        Standing::Member,
        "spam"
    ));
    assert!(!flagged_with(&[""], usual(), Standing::Member, "anything"));
    assert!(!flagged_with(&["", ""], usual(), Standing::Member, ""));
    assert!(flagged_with(
        &["", "spam"],
        usual(),
        Standing::Member,
        "spam"
    ));
    assert!(!flagged_with(&[], usual(), Standing::Member, "spam"));
}

#[test]
fn owner_is_not_checked_but_trusted_is() {
    for kind in [VenueKind::Group, VenueKind::Private] {
        let mut watch = ctx();
        watch.moderation.keywords = vec!["spam".to_string()];
        let mut owner = msg(Standing::Owner, kind);
        owner.text = "spam".to_string();
        assert!(judge(&owner, &watch, at(12, 0)).flags.is_empty());
        let mut trusted = msg(Standing::Trusted, kind);
        trusted.text = "spam".to_string();
        assert_eq!(judge(&trusted, &watch, at(12, 0)).flags, [Flag::Moderation]);
    }
}

#[test]
fn base64_long_enough_is_decoded() {
    // `please buy spam today`。
    assert!(flagged("look: cGxlYXNlIGJ1eSBzcGFtIHRvZGF5 ok"));
    assert!(flagged("看看cGxlYXNlIGJ1eSBzcGFtIHRvZGF5吧"));
    // `SPAM`：解出来的也不分 ASCII 的大小写。
    assert!(flagged("U1BBTQ=="));
    // 中文关键词：`这是违规内容`。
    let words = ["违规"];
    assert!(flagged_with(
        &words,
        usual(),
        Standing::Member,
        "6L+Z5piv6L+d6KeE5YaF5a65"
    ));
    // 明文、base64 都命中，也只插一面旗。
    assert!(flagged("spam c3BhbQ=="));
}

#[test]
fn base64_shorter_than_min_chars_is_skipped() {
    // `c3BhbQ==` 连 `=` 是 8 个字符。
    let at_least = |min_chars| Base64 {
        min_chars,
        ..usual()
    };
    assert!(flagged_with(
        &["spam"],
        at_least(8),
        Standing::Member,
        "c3BhbQ=="
    ));
    assert!(!flagged_with(
        &["spam"],
        at_least(9),
        Standing::Member,
        "c3BhbQ=="
    ));
    // 不带 `=` 是 6 个字符，长度不是 4 的倍数照样解。
    assert!(flagged_with(
        &["spam"],
        at_least(6),
        Standing::Member,
        "c3BhbQ"
    ));
    assert!(!flagged_with(
        &["spam"],
        at_least(7),
        Standing::Member,
        "c3BhbQ"
    ));
}

#[test]
fn base64_is_cut_at_other_chars() {
    // 中间夹了别的字，两段各解各的，都凑不出 `spam`。
    let short = Base64 {
        min_chars: 1,
        ..usual()
    };
    assert!(!flagged_with(
        &["spam"],
        short,
        Standing::Member,
        "c3Bh!bQ=="
    ));
    assert!(!flagged_with(
        &["spam"],
        short,
        Standing::Member,
        "c3Bh bQ=="
    ));
    // `=` 只在一段的末尾：后面再接的是另一段。
    assert!(flagged_with(
        &["spam"],
        short,
        Standing::Member,
        "c3BhbQ==c3BhbQ=="
    ));
    assert!(!flagged_with(
        &["spam"],
        short,
        Standing::Member,
        "c3Bh=bQ=="
    ));
}

#[test]
fn base64_mostly_unprintable_is_skipped() {
    // `\x01\x02\x03\x04\x05\x06spam`：十个字符里四个可打印，千分之四百。
    let text = "AQIDBAUGc3BhbQ==";
    let printable = |printable| Base64 {
        printable,
        ..usual()
    };
    assert!(!flagged_with(
        &["spam"],
        printable(900),
        Standing::Member,
        text
    ));
    assert!(flagged_with(
        &["spam"],
        printable(400),
        Standing::Member,
        text
    ));
    assert!(!flagged_with(
        &["spam"],
        printable(401),
        Standing::Member,
        text
    ));
    assert!(flagged_with(
        &["spam"],
        printable(0),
        Standing::Member,
        text
    ));
}

#[test]
fn base64_invalid_utf8_is_skipped() {
    // `\xffspam`：解出来的字节读不成 UTF-8，整段不要，不查（施工时定的第 20 条；O-5 原来换成替换字符照查）。
    assert!(!flagged("/3NwYW0="));
    // 读得成的照查：`spam`。
    assert!(flagged("/3NwYW0= c3BhbQ=="));
}

#[test]
fn base64_only_looks_at_first_max_chars() {
    // `aaaaaaaaaaspam`：十四个字符，`spam` 在第 11 到 14 个。
    let text = "YWFhYWFhYWFhYXNwYW0=";
    let first = |max_chars| Base64 {
        max_chars,
        ..usual()
    };
    assert!(flagged_with(&["spam"], first(14), Standing::Member, text));
    assert!(!flagged_with(&["spam"], first(13), Standing::Member, text));
    assert!(flagged_with(&["aaaa"], first(4), Standing::Member, text));
    assert!(!flagged_with(&["aaaa"], first(3), Standing::Member, text));
}

#[test]
fn base64_ratio_counts_only_the_first_max_chars() {
    // `spam\x01\x02\x03\x04\x05\x06`：只看前四个，全可打印；看全十个，千分之四百。
    let text = "c3BhbQECAwQFBg==";
    let first = |max_chars| Base64 {
        max_chars,
        ..usual()
    };
    assert!(flagged_with(&["spam"], first(4), Standing::Member, text));
    assert!(!flagged_with(&["spam"], first(10), Standing::Member, text));
}

#[test]
fn base64_owner_is_not_checked() {
    assert!(!flagged_with(
        &["spam"],
        usual(),
        Standing::Owner,
        "c3BhbQ=="
    ));
}
