use super::*;

#[test]
fn each_line_reads_back_as_written() {
    for ready in [
        Ready::Ready,
        Ready::Running,
        Ready::Failed("找不到资源目录".to_string()),
    ] {
        let line = ready.line();
        assert!(
            line.ends_with('\n') && line.matches('\n').count() == 1,
            "{line:?}"
        );
        assert_eq!(Ready::parse(&line), ready);
    }
}

#[test]
fn a_reason_stays_on_one_line() {
    let line = Ready::Failed("第一行\r\n第二行".to_string()).line();
    assert_eq!(line, "error 第一行  第二行\n");
}

#[test]
fn a_line_not_understood_is_a_failure_that_says_it() {
    assert_eq!(
        Ready::parse("panicked at main.rs\n"),
        Ready::Failed("panicked at main.rs".to_string())
    );
}
