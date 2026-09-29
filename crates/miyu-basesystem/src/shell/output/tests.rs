use super::*;

/// 一段段读进去。
fn captured(chunks: &[&[u8]]) -> Capture {
    let mut capture = Capture::default();
    for chunk in chunks {
        capture.push(chunk);
    }
    capture
}

/// 编号的行，从 `from` 到 `to`，每行 `width` 个字宽（不算换行）。
fn numbered(from: usize, to: usize, width: usize) -> String {
    (from..=to).map(|n| format!("{n:0width$}\n")).collect()
}

#[test]
fn counts_lines_and_characters() {
    let capture = captured(&[b"a\n\xe4\xb8", b"\xad\n", b"end"]);
    assert_eq!(capture.lines(), 3, "最后一行没有换行也算一行");
    assert_eq!(capture.shown(), Shown::Whole("a\n中\nend".into()));
    assert!(!capture.is_empty());
    assert!(captured(&[]).is_empty());
    assert_eq!(captured(&[b"x\n"]).lines(), 1);
}

#[test]
fn windows_line_ends_become_plain_ones() {
    assert_eq!(
        captured(&[b"a\r\nb\r", b"\nc\r"]).shown(),
        Shown::Whole("a\nb\nc\r".into()),
        "单独的 \\r 不动"
    );
}

#[test]
fn memory_keeps_only_the_head_and_the_tail() {
    let chunk = vec![b'x'; 10_000];
    let mut capture = Capture::default();
    for _ in 0..100 {
        capture.push(&chunk);
    }
    assert_eq!(capture.head.len(), KEEP);
    assert_eq!(capture.tail.len(), KEEP);
    assert_eq!(capture.bytes, 1_000_000);
    assert_eq!(capture.chars, 1_000_000);
}

#[test]
fn long_output_keeps_its_start_and_end_at_line_ends() {
    // 3000 行，每行 20 个字加一个换行：63000 个字，没丢，截的是整段。
    let text = numbered(1, 3000, 20);
    let capture = captured(&[text.as_bytes()]);
    let Shown::Cut {
        head,
        tail,
        omitted,
        total,
    } = capture.shown()
    else {
        panic!("该截")
    };
    assert_eq!(total, 63_000);
    assert!(head.starts_with(&numbered(1, 1, 20)), "{}", &head[..40]);
    assert!(tail.ends_with(&numbered(3000, 3000, 20)));
    assert!(head.ends_with('\n') && tail.ends_with('\n'));
    // 截在行尾：头是整行，尾也从一行的开头起。
    assert_eq!(head.len() % 21, 0);
    assert_eq!(tail.len() % 21, 0);
    assert!(head.chars().count() <= LIMIT / 2 && tail.chars().count() <= LIMIT / 2);
    assert_eq!(
        omitted,
        total - (head.chars().count() + tail.chars().count()) as u64
    );
    // 截在行尾只少留不到一行。
    assert!(head.chars().count() > LIMIT / 2 - 21);
    assert!(tail.chars().count() > LIMIT / 2 - 21);
}

#[test]
fn a_dropped_middle_is_counted_too() {
    // 一百多万个字：中间的已经丢了，头尾照样截在行尾，数的是全部。
    let text = numbered(1, 100_000, 10);
    let mut capture = Capture::default();
    for piece in text.as_bytes().chunks(8192) {
        capture.push(piece);
    }
    let Shown::Cut {
        head,
        tail,
        omitted,
        total,
    } = capture.shown()
    else {
        panic!("该截")
    };
    assert_eq!(total, 1_100_000);
    assert!(head.starts_with(&numbered(1, 1, 10)));
    assert!(tail.ends_with(&numbered(100_000, 100_000, 10)));
    assert_eq!(tail.len() % 11, 0, "尾从一行的开头起：丢了的那半行不留");
    assert_eq!(
        omitted,
        total - (head.chars().count() + tail.chars().count()) as u64
    );
}

#[test]
fn one_giant_line_is_cut_by_characters() {
    let text = "字".repeat(40_000);
    let Shown::Cut {
        head,
        tail,
        omitted,
        total,
    } = captured(&[text.as_bytes()]).shown()
    else {
        panic!("该截")
    };
    assert_eq!(head.chars().count(), LIMIT / 2);
    assert_eq!(tail.chars().count(), LIMIT / 2);
    assert_eq!((omitted, total), (10_000, 40_000));
}

#[test]
fn exactly_the_limit_is_whole() {
    let text = "x".repeat(LIMIT);
    assert_eq!(captured(&[text.as_bytes()]).shown(), Shown::Whole(text));
    let over = "x".repeat(LIMIT + 1);
    assert!(matches!(
        captured(&[over.as_bytes()]).shown(),
        Shown::Cut { omitted: 1, .. }
    ));
}

#[test]
fn a_character_split_across_reads_waits_for_its_end() {
    let mut decoder = Decoder::default();
    // 「中」是 e4 b8 ad，「😀」是 f0 9f 98 80。
    assert_eq!(decoder.push(b"a\xe4"), "a");
    assert_eq!(decoder.push(b"\xb8"), "");
    assert_eq!(decoder.push(b"\xad\xf0\x9f"), "中");
    assert_eq!(decoder.push(b"\x98\x80b"), "😀b");
    // 解不开的照样往下走。
    assert_eq!(decoder.push(b"\xff c"), "\u{fffd} c");
    // 读完了还没配齐的，换成 U+FFFD。
    assert_eq!(decoder.push(b"\xe4\xb8"), "");
    assert_eq!(decoder.finish(), "\u{fffd}");
    assert_eq!(decoder.finish(), "");
}

#[test]
fn unfinished_counts_only_a_real_start() {
    assert_eq!(unfinished(b""), 0);
    assert_eq!(unfinished(b"ab"), 0);
    assert_eq!(unfinished(b"\xe4\xb8\xad"), 0, "配齐了");
    assert_eq!(unfinished(b"\xe4\xb8"), 2);
    assert_eq!(unfinished(b"\xf0\x9f\x98"), 3);
    assert_eq!(unfinished(b"\xc3"), 1);
    // 接续字节多过三个的，不是一个字的开头：交给解码换成 U+FFFD。
    assert_eq!(unfinished(b"\x80\x80\x80"), 0);
}

#[test]
fn a_line_break_too_far_from_the_cut_is_not_used() {
    // 头：唯一的换行在最前面，截在它后面就只剩几个字了，照字数截。
    let text = format!("line1\n{}", "x".repeat(40_000));
    let Shown::Cut { head, .. } = captured(&[text.as_bytes()]).shown() else {
        panic!("该截")
    };
    assert_eq!(head.chars().count(), LIMIT / 2);
    // 尾：唯一的换行在最后面，从它后面起就只剩几个字了，照字数截。
    let text = format!("{}\nend\n", "x".repeat(40_000));
    let Shown::Cut { tail, .. } = captured(&[text.as_bytes()]).shown() else {
        panic!("该截")
    };
    assert_eq!(tail.chars().count(), LIMIT / 2);
}

/// 尾巴那一段正好从一行的开头起：照原样，这一行留着（施工 4-9 再补二：原来多丢了这一行）。
#[test]
fn a_tail_that_starts_at_a_line_start_keeps_that_line() {
    // 4000 行，每行 9 个字加一个换行：后 15000 个字正好从第 2501 行的开头起。
    let text = numbered(1, 4000, 9);
    let Shown::Cut { tail, .. } = captured(&[text.as_bytes()]).shown() else {
        panic!("该截")
    };
    assert_eq!(tail.chars().count(), LIMIT / 2);
    assert!(
        tail.starts_with(&numbered(2501, 2501, 9)),
        "{}",
        &tail[..20]
    );
}

/// 后台命令的输出一段段换行尾（施工 7-3）：`\r\n` 切在两段中间的也换，单独的 `\r` 不动，最后留着的交回来。
#[test]
fn line_ends_are_made_unix_across_pieces() {
    let mut crlf = Crlf::default();
    let pieces = ["a\r", "\nb\r", "c\r", "", "\r\n", "d\r"];
    let mut got: String = pieces.iter().map(|piece| crlf.push(piece)).collect();
    got.push_str(&crlf.finish());
    let whole: String = pieces.concat();
    assert_eq!(got, whole.replace("\r\n", "\n"), "和一整段换出来的一样");
    assert_eq!(got, "a\nb\rc\r\nd\r");
    assert_eq!(crlf.finish(), "", "交回过了就没有了");
}
