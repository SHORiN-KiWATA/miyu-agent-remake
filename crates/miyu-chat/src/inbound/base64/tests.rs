//! 自己写的 base64（施工单「风险」第 2 条）：填充、长度不是 4 的倍数、被别的字截开、整个字母表。

use super::{decode, segments};

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
