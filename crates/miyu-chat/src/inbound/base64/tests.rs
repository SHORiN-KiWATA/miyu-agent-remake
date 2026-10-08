//! 自己写的 base64（O-5 施工单「风险」第 2 条）：填充、长度不是 4 的倍数、被别的字截开、整个字母表。解出来的字
//! （`chat.md` 第二条「守着它的」，O-12 下）：没有、一段、两段、重复的只留一个、不够长的、不可打印的、读不成 UTF-8 的、
//! 照出现的先后。

use super::{Base64, decode, segments};

/// 测试随手定的三个数：至少 8 个字符、看前 100 个、可打印的至少九成。
fn usual() -> Base64 {
    Base64 {
        min_chars: 8,
        max_chars: 100,
        printable: 900,
    }
}

/// 照 [`usual`] 的三个数解 `text` 里的 base64。
fn reveal(text: &str) -> Option<String> {
    usual().reveal(text)
}

#[test]
fn decodes_with_and_without_padding() {
    assert_eq!(decode("TWFu"), b"Man");
    assert_eq!(decode("TWE="), b"Ma");
    assert_eq!(decode("TQ=="), b"M");
    assert_eq!(decode("TWE"), b"Ma");
    assert_eq!(decode("TQ"), b"M");
    assert_eq!(decode("c3BhbQ=="), b"spam");
    assert_eq!(decode(""), b"");
}

#[test]
fn a_lone_last_char_is_dropped() {
    // 剩一个字符凑不出一个字节。
    assert_eq!(decode("T"), b"");
    assert_eq!(decode("TWFuT"), b"Man");
    assert_eq!(decode("TWFuTQ"), b"ManM");
}

#[test]
fn decodes_the_whole_alphabet() {
    // 0x00 0x10 0x83 0x10 0x51 0x87 ... 依次是 0 到 63 这六十四个六位数。
    let all = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = decode(all);
    assert_eq!(bytes.len(), 48);
    let mut sextets = Vec::new();
    for chunk in bytes.chunks(3) {
        let word = chunk
            .iter()
            .fold(0_u32, |word, &byte| word << 8 | u32::from(byte));
        sextets.extend((0..4).rev().map(|at| (word >> (6 * at)) & 0x3f));
    }
    assert_eq!(sextets, (0..64).collect::<Vec<u32>>());
    assert_eq!(decode("+/+/"), [0xfb, 0xff, 0xbf]);
}

#[test]
fn segments_are_runs_with_trailing_padding() {
    assert_eq!(segments("ab+/9 c=="), ["ab+/9", "c=="]);
    assert_eq!(segments("c3Bh=bQ=="), ["c3Bh=", "bQ=="]);
    assert_eq!(segments("看看c3BhbQ==吧"), ["c3BhbQ=="]);
    assert_eq!(segments("a-b_c.d"), ["a", "b", "c", "d"]);
    assert_eq!(segments("== = !"), Vec::<&str>::new());
    assert_eq!(segments(""), Vec::<&str>::new());
    assert_eq!(segments("x==="), ["x==="]);
}

#[test]
fn reveal_nothing() {
    assert_eq!(reveal(""), None);
    assert_eq!(reveal("你好，今天吃什么"), None);
    // 都不够长：一个词也不解。
    assert_eq!(reveal("hello world, how are you"), None);
    // 够长、解出来可打印的不够九成的不算：`helloworld` 解出来七个字符，一个是换行。
    assert_eq!(reveal("say helloworld"), None);
}

#[test]
fn reveal_one() {
    // `please buy spam today`；前后不够长的词不解。
    let text = "look: cGxlYXNlIGJ1eSBzcGFtIHRvZGF5 ok";
    assert_eq!(reveal(text).as_deref(), Some("please buy spam today"));
    assert_eq!(reveal("看看c3BhbQ==吧").as_deref(), Some("spam"));
}

#[test]
fn reveal_two_in_order() {
    // `spam`、`Hello world`：照出现的先后用换行接起来。
    let text = "c3BhbQ== and SGVsbG8gd29ybGQ=";
    assert_eq!(reveal(text).as_deref(), Some("spam\nHello world"));
    let text = "SGVsbG8gd29ybGQ= and c3BhbQ==";
    assert_eq!(reveal(text).as_deref(), Some("Hello world\nspam"));
}

#[test]
fn reveal_keeps_one_of_each() {
    // 解出来一样的只留第一个：不带 `=` 的 `c3BhbQ` 也是 `spam`，比的是解出来的字。
    let text = "c3BhbQ== SGVsbG8gd29ybGQ= c3BhbQ== c3BhbQ";
    let six = Base64 {
        min_chars: 6,
        ..usual()
    };
    assert_eq!(six.reveal(text).as_deref(), Some("spam\nHello world"));
    // 只看前 `max_chars` 个字符，截完一样的也只留一个：`spam`、`spammer`。
    let four = Base64 {
        max_chars: 4,
        ..usual()
    };
    assert_eq!(
        four.reveal("c3BhbQ== c3BhbW1lcg==").as_deref(),
        Some("spam")
    );
}

#[test]
fn reveal_skips_short_segments() {
    // `c3BhbQ==` 连 `=` 是 8 个字符。
    let at_least = |min_chars| Base64 {
        min_chars,
        ..usual()
    };
    assert_eq!(at_least(8).reveal("c3BhbQ==").as_deref(), Some("spam"));
    assert_eq!(at_least(9).reveal("c3BhbQ=="), None);
    // 不够长的跳过，够长的照留。
    let text = "c3BhbQ== SGVsbG8gd29ybGQ=";
    assert_eq!(at_least(9).reveal(text).as_deref(), Some("Hello world"));
}

#[test]
fn reveal_skips_unprintable() {
    // `\x01\x02\x03\x04\x05\x06spam`：十个字符里四个可打印，千分之四百。
    let text = "AQIDBAUGc3BhbQ== c3BhbQ==";
    let printable = |printable| Base64 {
        printable,
        ..usual()
    };
    assert_eq!(printable(401).reveal(text).as_deref(), Some("spam"));
    let both = "\u{1}\u{2}\u{3}\u{4}\u{5}\u{6}spam\nspam";
    assert_eq!(printable(400).reveal(text).as_deref(), Some(both));
}

#[test]
fn reveal_drops_segments_that_are_not_utf8() {
    // 长数字串、哈希这类解出来的字节读不成 UTF-8：整段不要，不换成替换字符（施工时定的第 20 条）。`12345678` 解出来是
    // `d7 6d f8 df 8e fc`，换成替换字符的话可打印的有十成，会被当成字交给判官。
    assert_eq!(reveal("12345678"), None);
    // 短一点的数字串，`min_chars` 放到够得着它也一样。
    let six = Base64 {
        min_chars: 6,
        ..usual()
    };
    assert_eq!(six.reveal("123456"), None);
    // `\xffspam`：坏一个字节也整段不要。
    assert_eq!(reveal("/3NwYW0="), None);
    // 读得成的照旧留下，多字节的字也是：`这是违规内容`。
    assert_eq!(
        reveal("6L+Z5piv6L+d6KeE5YaF5a65").as_deref(),
        Some("这是违规内容")
    );
    // 一段坏的、一段好的：只留好的。
    let text = "12345678 c3BhbQ== /3NwYW0=";
    assert_eq!(reveal(text).as_deref(), Some("spam"));
}
