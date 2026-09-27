//! 按行读：编码、二进制、分页、行号、截长行、64 KiB 的上限。

use std::io::Write;

use super::*;
use crate::common::OUTPUT_BYTES;

/// 一个用完就删的临时文件，内容是 `bytes`。
struct Temp(std::path::PathBuf);

impl Temp {
    fn with(bytes: &[u8]) -> Temp {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("miyu-lines-{}-{n}", std::process::id()));
        std::fs::File::create(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        Temp(path)
    }

    fn page(&self, offset: u64, limit: u64) -> Page {
        read(File::open(&self.0).unwrap(), offset, limit).unwrap()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn lines(text: &str, from: u64, to: u64, total: u64) -> Page {
    Page::Lines {
        text: text.to_string(),
        from,
        to,
        total,
    }
}

#[test]
fn lines_come_with_a_number_and_a_tab() {
    let file = Temp::with(b"one\ntwo\r\nthree");
    assert_eq!(
        file.page(1, 2000),
        lines("1\tone\n2\ttwo\n3\tthree\n", 1, 3, 3)
    );
    assert_eq!(file.page(2, 1), lines("2\ttwo\n", 2, 2, 3));
    assert_eq!(file.page(4, 10), Page::PastEnd { total: 3 });
    // 以换行结尾的，最后一行后面没有空行。
    assert_eq!(Temp::with(b"a\n").page(1, 10), lines("1\ta\n", 1, 1, 1));
}

#[test]
fn empty_binary_and_bom() {
    assert_eq!(Temp::with(b"").page(1, 10), Page::Empty);
    assert_eq!(Temp::with(b"\xEF\xBB\xBF").page(1, 10), Page::Empty);
    assert_eq!(Temp::with(b"\x7FELF\0\0\x01").page(1, 10), Page::Binary);
    assert_eq!(
        Temp::with(b"\xEF\xBB\xBFhi\n").page(1, 10),
        lines("1\thi\n", 1, 1, 1),
        "UTF-8 的 BOM 不显示"
    );
}

#[test]
fn utf16_both_byte_orders() {
    let text = "你好\r\nMiyu\n";
    let little: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let big: Vec<u8> = [0xFE, 0xFF]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_be_bytes))
        .collect();
    let expected = lines("1\t你好\n2\tMiyu\n", 1, 2, 2);
    assert_eq!(Temp::with(&little).page(1, 10), expected);
    assert_eq!(Temp::with(&big).page(1, 10), expected);
    assert_eq!(Temp::with(&[0xFF, 0xFE]).page(1, 10), Page::Empty);
}

#[test]
fn a_long_line_is_cut() {
    let long = "字".repeat(LINE_CHARS + 5);
    let Page::Lines { text, .. } = Temp::with(long.as_bytes()).page(1, 10) else {
        panic!("该读得出来");
    };
    let shown = text.trim_start().trim_start_matches("1\t").trim_end();
    assert_eq!(shown.chars().count(), LINE_CHARS + 1);
    assert!(shown.ends_with('…'));
}

#[test]
fn a_page_stops_at_the_output_limit() {
    let line = "x".repeat(1000);
    let body: String = (0..200).map(|_| format!("{line}\n")).collect();
    let Page::Lines {
        text,
        from,
        to,
        total,
    } = Temp::with(body.as_bytes()).page(1, 2000)
    else {
        panic!("该读得出来");
    };
    assert_eq!((from, total), (1, 200));
    assert!(to < total, "到了 64 KiB 就停：{to}");
    assert!(text.len() <= OUTPUT_BYTES);
    assert_eq!(text.lines().count() as u64, to);
}
