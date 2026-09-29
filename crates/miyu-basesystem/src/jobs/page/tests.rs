use std::io::Cursor;

use super::*;

fn page(text: &str, offset: u64) -> Page {
    Page::read(Box::new(Cursor::new(text.to_string())), offset)
}

#[test]
fn lines_are_counted_with_or_without_the_last_newline() {
    assert_eq!(page("", 1), Page::default());
    let with = page("a\nb\n", 1);
    let without = page("a\nb", 1);
    assert_eq!((with.total, with.lines), (2, Some((1, 2))));
    assert_eq!(with, without, "最后一段没有换行的也算一行");
    assert_eq!(page("\n\n", 1).text, "\n\n", "空行照样算");
}

#[test]
fn the_page_stops_on_a_whole_line_and_keeps_counting() {
    let line = "x".repeat(LIMIT / 2);
    let text = format!("{line}\n{line}\ny\nz\n");
    let first = page(&text, 1);
    assert_eq!(first.lines, Some((1, 2)), "正好到上限的那一行照给");
    assert_eq!(first.total, 4, "停了照样数到结尾");
    let rest = page(&text, 3);
    assert_eq!((rest.text.as_str(), rest.lines), ("y\nz\n", Some((3, 4))));
    let over = page(&format!("{line}\n{line}x\n"), 1);
    assert_eq!(over.lines, Some((1, 1)), "多一个字就停在前一行");
}

#[test]
fn one_line_longer_than_the_page_is_cut() {
    let long = "字".repeat(LIMIT + 5);
    let got = page(&format!("{long}\nnext\n"), 1);
    assert_eq!(got.lines, Some((1, 1)));
    assert_eq!(got.text, format!("{}…\n", "字".repeat(LIMIT)));
    assert_eq!(page(&format!("{long}\nnext\n"), 2).text, "next\n");
}

#[test]
fn an_offset_past_the_end_has_no_lines() {
    let got = page("a\nb\n", 3);
    assert_eq!((got.lines, got.total, got.text.as_str()), (None, 2, ""));
}

#[test]
fn a_line_exactly_as_long_as_the_page_is_whole() {
    let line = "字".repeat(LIMIT);
    let got = page(&format!("{line}\nnext\n"), 1);
    assert_eq!(got.text, format!("{line}\n"), "正好到上限的不截、不补 `…`");
    assert_eq!(got.lines, Some((1, 1)));
}
