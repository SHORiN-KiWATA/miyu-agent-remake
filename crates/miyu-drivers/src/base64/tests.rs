//! RFC 4648 第十节的七个测试值，加上用到 `+`、`/` 的两个字节。

use super::*;

#[test]
fn the_rfc_test_vectors() {
    for (input, output) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(encode(input.as_bytes()), output, "{input:?}");
    }
}

#[test]
fn the_last_two_letters_of_the_alphabet() {
    assert_eq!(encode(&[0xfb, 0xff]), "+/8=");
    assert_eq!(encode(&[0x00, 0x10, 0x83]), "ABCD");
}
