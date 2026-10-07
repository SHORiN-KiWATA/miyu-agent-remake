//! Markdown 转纯文本（`chat.md` 第五条「守着它的」）：代码块、标题、引用、加粗、反引号、链接三种、单个 `*`；末尾的
//! 空白；一长串不成对的 `[` 不变慢。

use super::plain;

#[test]
fn fenced_code_is_kept_verbatim() {
    let markdown = "看这段：\n```rust\nlet **x** = `1`;\n  # 不是标题\n```\n**完**";
    assert_eq!(
        plain(markdown),
        "看这段：\nlet **x** = `1`;\n  # 不是标题\n完"
    );
}

#[test]
fn tilde_and_indented_fences_toggle_too() {
    let markdown = "  ~~~\n__a__\n  ~~~\n__b__\n```\n`c`";
    // 没关上的代码块一直到末尾。
    assert_eq!(plain(markdown), "__a__\nb\n`c`");
}

#[test]
fn headings_lose_their_hashes() {
    assert_eq!(plain("# 一\n## 二\n   ###   三 **粗**"), "一\n二\n三 粗");
}

#[test]
fn quotes_lose_their_marker() {
    assert_eq!(
        plain("> 引一句\n  > 缩进的\n>没空格"),
        "引一句\n缩进的\n>没空格"
    );
}

#[test]
fn bold_underline_and_backticks_are_removed() {
    assert_eq!(
        plain("**粗** __下划__ `代码` ``两个``"),
        "粗 下划 代码 两个"
    );
}

#[test]
fn links_keep_text_and_address() {
    assert_eq!(
        plain("看[文档](https://x.example/a)吧"),
        "看文档 (https://x.example/a)吧"
    );
}

#[test]
fn links_with_empty_or_same_address_keep_only_text() {
    assert_eq!(plain("[空]()"), "空");
    assert_eq!(
        plain("[https://x.example](https://x.example)"),
        "https://x.example"
    );
}

#[test]
fn broken_links_stay_as_written() {
    assert_eq!(plain("[a] b"), "[a] b");
    assert_eq!(plain("[a](b"), "[a](b");
    assert_eq!(plain("[[a](b)"), "[a (b)");
}

#[test]
fn single_star_lists_and_newlines_stay() {
    let markdown = "2 * 3 = 6\n*斜体*\n- 一\n- 二\n\n1. 三";
    assert_eq!(plain(markdown), markdown);
}

#[test]
fn indentation_of_plain_lines_stays() {
    assert_eq!(plain("  - 缩进的列表"), "  - 缩进的列表");
}

#[test]
fn trailing_blank_is_removed() {
    assert_eq!(plain("好的  \n\n \n"), "好的");
    assert_eq!(plain(""), "");
}

#[test]
fn many_unmatched_brackets_stay_linear() {
    // 旧版每个 `[` 往后找 `]` 是平方的：16000 个要 358ms。只断言结果，慢了靠眼看。
    let brackets = "[".repeat(50_000);
    assert_eq!(plain(&brackets), brackets);
    let mixed = "[a]".repeat(20_000);
    assert_eq!(plain(&mixed), mixed);
}
