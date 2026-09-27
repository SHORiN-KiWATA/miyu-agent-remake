//! 文件的写法：认编码、BOM、第一处换行；照原来的写法写回，UTF-16 写得回去、解得回来；几行。

use super::*;

#[test]
fn the_style_is_read_from_the_old_bytes() {
    let cases: [(&[u8], Style); 7] = [
        (b"a\nb\n", Style::fresh()),
        (
            b"a\r\nb\n",
            Style {
                crlf: true,
                ..Style::fresh()
            },
        ),
        (b"a\nb\r\n", Style::fresh()),
        (
            b"\xEF\xBB\xBFa\r\n",
            Style {
                encoding: Encoding::Utf8,
                bom: true,
                crlf: true,
            },
        ),
        (
            b"\xFF\xFEa\x00\r\x00\n\x00",
            Style {
                encoding: Encoding::Utf16Le,
                bom: true,
                crlf: true,
            },
        ),
        (
            b"\xFE\xFF\x00a\x00\n",
            Style {
                encoding: Encoding::Utf16Be,
                bom: true,
                crlf: false,
            },
        ),
        (b"no line break at all", Style::fresh()),
    ];
    for (bytes, style) in cases {
        assert_eq!(Style::of(bytes), style, "{bytes:?}");
    }
}

#[test]
fn text_is_written_back_the_same_way() {
    let crlf = Style {
        crlf: true,
        ..Style::fresh()
    };
    assert_eq!(
        crlf.encode("a\nb\r\nc"),
        b"a\r\nb\r\nc",
        "单个换行换成 CRLF，原来的不重复换"
    );
    assert_eq!(
        Style::fresh().encode("a\r\nb\n"),
        b"a\r\nb\n",
        "不是 CRLF 的照给的写"
    );
    let bom = Style {
        bom: true,
        ..Style::fresh()
    };
    assert_eq!(bom.encode("a"), b"\xEF\xBB\xBFa");
    for style in [
        Style {
            encoding: Encoding::Utf16Le,
            bom: true,
            crlf: false,
        },
        Style {
            encoding: Encoding::Utf16Be,
            bom: true,
            crlf: true,
        },
    ] {
        let bytes = style.encode("中文 é\n");
        assert_eq!(Style::of(&bytes), style, "写回去的认得出同样的写法");
        let back = decode(style.encoding, &bytes);
        let want = if style.crlf {
            "中文 é\r\n"
        } else {
            "中文 é\n"
        };
        assert_eq!(back, want);
    }
    assert_eq!(
        Style {
            encoding: Encoding::Utf16Le,
            bom: true,
            crlf: false
        }
        .encode("a"),
        b"\xFF\xFEa\x00"
    );
}

#[test]
fn decoding_drops_the_bom_and_keeps_going_past_bad_bytes() {
    assert_eq!(decode(Encoding::Utf8, b"\xEF\xBB\xBFhi"), "hi");
    assert_eq!(decode(Encoding::Utf8, b"a\xFFb"), "a\u{FFFD}b");
    assert_eq!(decode(Encoding::Utf16Le, b"\xFF\xFE"), "");
    assert_eq!(
        decode(Encoding::Utf16Le, b"\xFF"),
        "",
        "短得连 BOM 都不够的，是空的"
    );
}

#[test]
fn lines_are_counted_like_read_counts_them() {
    assert_eq!(line_count(""), 0);
    assert_eq!(line_count("a"), 1);
    assert_eq!(line_count("a\n"), 1);
    assert_eq!(line_count("a\nb"), 2);
    assert_eq!(line_count("a\r\nb\r\n"), 2);
    assert_eq!(line_count("\n"), 1);
}

#[test]
fn strict_decoding_refuses_what_cannot_be_written_back() {
    let utf8 = Style::fresh();
    assert_eq!(utf8.strict(b"\xEF\xBB\xBFhi").as_deref(), Some("hi"));
    assert_eq!(utf8.strict(b"a\xFFb"), None);
    let utf16 = Style::of(b"\xFF\xFEa\x00");
    assert_eq!(utf16.strict(b"\xFF\xFEa\x00").as_deref(), Some("a"));
    assert_eq!(utf16.strict(b"\xFF\xFEa\x00b"), None, "落单的一个字节");
    assert_eq!(utf16.strict(b"\xFF\xFE\x00\xD8"), None, "落单的代理项");
    assert_eq!(utf16.strict(b"\xFF"), None, "连 BOM 都不够");
}
