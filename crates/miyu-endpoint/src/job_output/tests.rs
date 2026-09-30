use std::io::{self, Cursor};

use miyu_kernel::id::CommandId;

use super::*;
use crate::wire::{LINE_LIMIT, result};

fn tail(text: &str, want: usize) -> Tail {
    Tail::read(Cursor::new(text.as_bytes().to_vec()), want)
}

fn got(output: &str, lines: u64, truncated: bool) -> Tail {
    Tail {
        output: output.to_string(),
        lines,
        truncated,
    }
}

#[test]
fn lines_are_counted_like_jobs_does() {
    assert_eq!(tail("", 200), Tail::default(), "空的一行都没有");
    assert_eq!(tail("a\nb\n", 200), got("a\nb\n", 2, false));
    assert_eq!(
        tail("a\nb", 200),
        got("a\nb", 2, false),
        "最后一段没有换行也算一行，原样交"
    );
    assert_eq!(tail("\n\n", 200), got("\n\n", 2, false), "空行照样算");
    assert_eq!(
        tail("a\r\nb\rc", 200),
        got("a\r\nb\rc", 2, false),
        "只照换行切"
    );
}

#[test]
fn only_the_last_lines_are_kept() {
    assert_eq!(tail("1\n2\n3\n", 2), got("2\n3\n", 3, true));
    assert_eq!(tail("1\n2\n3", 1), got("3", 3, true));
    assert_eq!(tail("1\n2\n3\n", 3), got("1\n2\n3\n", 3, false));
    assert_eq!(tail("1\n2\n3\n", 2000), got("1\n2\n3\n", 3, false));
}

#[test]
fn over_the_cap_whole_lines_go_from_the_front() {
    let line = |c: &str| format!("{}\n", c.repeat(59_999));
    let (a, b, c) = (line("a"), line("b"), line("c"));
    assert_eq!(
        tail(&format!("{a}{b}{c}"), 200),
        got(&format!("{b}{c}"), 3, true),
        "两行十二万字节放得下，三行放不下"
    );
}

#[test]
fn the_cap_is_128_kib() {
    // 正好 131072 字节（连换行）的一行整行给；前面再多一行就放不下，整行去掉。
    let full = format!("{}\n", "x".repeat(131_071));
    assert_eq!(tail(&full, 200), got(&full, 1, false));
    assert_eq!(tail(&format!("a\n{full}"), 200), got(&full, 2, true));
    // 多一个字节的一行，只留末尾 131072 字节。
    let over = format!("y{full}");
    assert_eq!(tail(&over, 200), got(&full, 1, true));
}

#[test]
fn a_last_line_over_the_cap_keeps_only_its_end() {
    let long = "y".repeat(200_000);
    assert_eq!(
        tail(&format!("a\n{long}"), 200),
        got(&"y".repeat(131_072), 2, true),
        "只交它的末尾，前面的行不接上"
    );
    // 比读的时候攒的两倍还长：读的时候已经去掉过前面。
    let huge = format!("{}\n", "z".repeat(3 * 131_072 + 5));
    assert_eq!(
        tail(&format!("a\n{huge}"), 200),
        got(&format!("{}\n", "z".repeat(131_071)), 2, true)
    );
}

#[test]
fn a_long_line_in_the_middle_goes_whole() {
    for n in [200_000, 3 * 131_072 + 5] {
        let text = format!("{}\na\nb\n", "x".repeat(n));
        assert_eq!(tail(&text, 200), got("a\nb\n", 3, true), "{n}");
    }
    // 截过的一行后面又来一行：就算两样加起来放得下（43690 个「字」连换行 131071 字节，再一个换行正好 131072），它也整行去掉。
    let text = format!("{}\n\n", "字".repeat(50_000));
    assert_eq!(tail(&text, 200), got("\n", 2, true));
}

#[test]
fn a_cut_starts_on_a_whole_char() {
    // 「字」三个字节：131072 不是三的倍数，末尾只留得下 43690 个字。
    let words = "字".repeat(50_000);
    assert_eq!(tail(&words, 200), got(&"字".repeat(43_690), 1, true));
    // 截下来 131070 字节，前面那一行 `a\n` 两个字节正好也放得下：照样不接上。
    assert_eq!(
        tail(&format!("a\n{words}"), 200),
        got(&"字".repeat(43_690), 2, true)
    );
    // 读的时候就去掉过前面的，开头也不是半个字。
    let words = "字".repeat(100_000);
    assert_eq!(tail(&words, 200), got(&"字".repeat(43_690), 1, true));
}

#[test]
fn a_cut_line_never_starts_with_half_a_char() {
    // 读的时候去掉过前面，开头剩下四字节的字的后三个字节：跳过它们，不换成三个 `�`。
    let mut raw = vec![0x9F, 0x98, 0x80];
    raw.extend_from_slice("😀".repeat(32_767).as_bytes());
    assert_eq!(fit(raw, true), ("😀".repeat(32_767), true));
}

#[test]
fn bytes_that_do_not_decode_become_replacement_chars() {
    let read = Tail::read(Cursor::new(vec![0xFF, b'\n', b'a']), 200);
    assert_eq!(read, got("\u{FFFD}\na", 2, false));
}

/// 读几段以后读不下去的源。
struct Broken(Vec<&'static [u8]>);

impl Read for Broken {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.0.pop() {
            Some(piece) => {
                buf[..piece.len()].copy_from_slice(piece);
                Ok(piece.len())
            }
            None => Err(io::Error::other("gone")),
        }
    }
}

#[test]
fn a_source_that_breaks_gives_what_was_read() {
    let read = Tail::read(Broken(vec![&b"c"[..], &b"a\nb\n"[..]]), 200);
    assert_eq!(read, got("a\nb\nc", 3, false));
}

#[test]
fn the_worst_reply_fits_on_one_line() {
    // 最坏的：每个字节都写成 `\u0001` 这样的六个，命令编号 128 个字节全是要转义的引号，行数最大，两个布尔都是 `false`。
    let id = CommandId::parse(&"\"".repeat(128)).unwrap();
    let worst = Tail {
        output: "\u{1}".repeat(CAP),
        lines: u64::MAX,
        truncated: false,
    };
    let line = result(&id, reply(worst, false));
    assert!(line.len() <= LINE_LIMIT, "{} 字节", line.len());
}

#[test]
fn the_reply_names_its_fields() {
    assert_eq!(
        result(
            &CommandId::parse("o1").unwrap(),
            reply(got("a\n", 1, false), true)
        ),
        r#"{"id":"o1","jsonrpc":"2.0","result":{"lines":1,"output":"a\n","running":true,"truncated":false}}"#
    );
}
