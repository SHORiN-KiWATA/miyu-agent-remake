use super::*;

const TABLE: &str = include_str!("../../../../resources/models/logos.json");

#[test]
fn the_shipped_table_reads_and_picks_a_source() {
    let table = LogoTable::parse(TABLE).expect("出厂的读得成");
    assert_eq!(
        table.source("deepseek"),
        (
            "https://cdn.jsdelivr.net/npm/@lobehub/icons-static-svg@1.95.1/icons/deepseek-color.svg"
                .to_string(),
            false
        )
    );
    assert_eq!(
        table.source("anthropic"),
        ("https://models.dev/logos/anthropic.svg".to_string(), true)
    );
    assert_eq!(
        table.probe(),
        "https://models.dev/logos/miyu-no-such-provider.svg"
    );
    assert!(LogoTable::parse("{\"models_dev\": \"x\"}").is_err());
}

#[test]
fn only_a_small_svg_that_is_not_the_default_is_taken() {
    let svg = b"<svg viewBox=\"0 0 1 1\"><path d=\"M0 0\"/></svg>";
    assert_eq!(
        accept(svg, None).as_deref(),
        Some(std::str::from_utf8(svg).unwrap())
    );
    let declared = b"<?xml version=\"1.0\"?>\n<svg></svg>";
    assert!(accept(declared, None).is_some(), "XML 声明认得");
    assert_eq!(accept(svg, Some(svg)), None, "默认图当没有");
    assert_eq!(accept(b"<html>not found</html>", None), None);
    assert_eq!(accept(b"<svg>", None), None, "没收尾的不收");
    assert_eq!(accept(&[0xff, 0xfe], None), None);
    let mut big = b"<svg>".to_vec();
    big.resize(LOGO_MAX + 1, b' ');
    big.extend_from_slice(b"</svg>");
    assert_eq!(accept(&big, None), None, "超过 32 KiB 的不收");
}
