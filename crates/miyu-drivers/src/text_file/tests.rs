use super::{LIMIT, is_text, shown};

#[test]
fn text_is_valid_utf8_without_nul() {
    assert!(is_text(b""), "空的也算");
    assert!(is_text("第一行\n第二行".as_bytes()));
    assert!(is_text(b"\xEF\xBB\xBFwith a bom"));
    assert!(!is_text(b"a\0b"), "有 NUL 的是二进制");
    assert!(!is_text(b"caf\xE9"), "Latin-1 不是 UTF-8");
    assert!(!is_text(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n"));
    // NUL 在很后面也算：看整份，不只看开头。
    let mut late = vec![b'a'; 100_000];
    late.push(0);
    assert!(!is_text(&late));
}

#[test]
fn only_the_first_64_kib_are_shown_cut_on_a_character() {
    let fits = "a".repeat(LIMIT);
    assert_eq!(shown(&fits), fits, "正好 64 KiB 的整份给");
    let over = "a".repeat(LIMIT + 1);
    assert_eq!(shown(&over).len(), LIMIT);
    // 「字」占三个字节：65,536 不是它的边界，退到 65,535。
    let chinese = "字".repeat(30_000);
    let cut = shown(&chinese);
    assert_eq!(cut.len(), LIMIT - 1);
    assert!(cut.chars().all(|c| c == '字'));
}
