use std::io::Write;

use super::*;

#[tokio::test]
async fn each_line_the_core_writes_is_heard() {
    for (line, heard) in [
        ("ready\n", Ready::Ready),
        ("running\n", Ready::Running),
        (
            "error 找不到资源目录\n",
            Ready::Failed("找不到资源目录".to_string()),
        ),
    ] {
        let (reader, mut writer) = io::pipe().expect("开得了管道");
        writer.write_all(line.as_bytes()).expect("写得进");
        assert_eq!(hear(reader, WAIT).await.expect("听得到"), heard);
    }
}

#[tokio::test]
async fn a_core_that_leaves_without_a_word_is_silent() {
    let (reader, writer) = io::pipe().expect("开得了管道");
    drop(writer);
    let error = hear(reader, WAIT).await.expect_err("没说话");
    assert!(matches!(error, StartError::Silent), "{error:?}");
}

#[tokio::test]
async fn a_core_that_says_nothing_for_too_long_times_out() {
    let (reader, writer) = io::pipe().expect("开得了管道");
    let error = hear(reader, Duration::from_millis(100))
        .await
        .expect_err("等太久");
    assert!(matches!(error, StartError::Timeout), "{error:?}");
    drop(writer);
}
