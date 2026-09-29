//! 起提升过的自己时的参数（`docs/blueprint/sandbox/windows.md`「怎么走」第 3 条）：照 Windows 命令行的规矩加引号，
//! 被拆回来的时候（`CommandLineToArgvW`、C 运行库）和原来一样。

use super::*;

/// 写成 UTF-16。
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// 这几个参数拼出来的一行，写回 `String` 好比较。
fn line(args: &[&str]) -> String {
    let args: Vec<Vec<u16>> = args.iter().map(|arg| wide(arg)).collect();
    String::from_utf16(&command_line(&args).expect("拼得成")).expect("是 UTF-16")
}

#[test]
fn plain_words_stay_as_they_are() {
    assert_eq!(line(&["sandbox", "setup"]), "sandbox setup");
    assert_eq!(
        line(&["--owner-home", r"C:\Users\me\.miyu"]),
        r"--owner-home C:\Users\me\.miyu",
        "不在引号前的反斜杠照原样"
    );
    assert_eq!(line(&[]), "");
}

#[test]
fn spaces_tabs_and_empty_words_are_quoted() {
    assert_eq!(
        line(&[r"C:\Users\John Smith\.miyu"]),
        r#""C:\Users\John Smith\.miyu""#
    );
    assert_eq!(line(&["a\tb"]), "\"a\tb\"");
    assert_eq!(line(&[""]), r#""""#);
    assert_eq!(line(&["a", "", "b"]), r#"a "" b"#);
}

#[test]
fn quotes_and_the_backslashes_before_them_are_escaped() {
    // 引号前加一个反斜杠；引号前原有的 n 个反斜杠变成 2n 个，再加那一个。
    assert_eq!(line(&[r#"a"b"#]), r#"a\"b"#);
    assert_eq!(line(&[r#"a\"b"#]), r#"a\\\"b"#);
    assert_eq!(line(&[r#"a\\"b"#]), r#"a\\\\\"b"#);
    assert_eq!(line(&[r#"say "hi""#]), r#""say \"hi\"""#);
}

#[test]
fn a_trailing_backslash_inside_quotes_is_doubled() {
    // 包起来的参数以反斜杠结尾：不加倍的话，它会把收尾的引号转义掉。
    assert_eq!(
        line(&[r"C:\Program Files\Miyu\"]),
        r#""C:\Program Files\Miyu\\""#
    );
    assert_eq!(line(&[r"a b\\"]), r#""a b\\\\""#);
    // 没包起来的，结尾的反斜杠照原样。
    assert_eq!(line(&[r"C:\Miyu\"]), r"C:\Miyu\");
}

#[test]
fn text_beyond_ascii_is_kept_unit_for_unit() {
    let args = [wide(r"C:\用户\美优 数据"), wide("𝄞")];
    let joined = command_line(&args).expect("拼得成");
    let mut expected = wide(r#""C:\用户\美优 数据""#);
    expected.push(u16::from(b' '));
    expected.extend(wide("𝄞"));
    assert_eq!(joined, expected);
    // 落单的代理项（Windows 的路径里可能有）也照原样。
    let lone = vec![vec![0xD800_u16, u16::from(b'x')]];
    assert_eq!(
        command_line(&lone).expect("拼得成"),
        vec![0xD800, u16::from(b'x')]
    );
}

#[test]
fn a_word_with_a_nul_cannot_be_passed() {
    let args = [wide("a\0b")];
    assert!(command_line(&args).is_err());
}
