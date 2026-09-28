//! 装到哪了那份记录（`docs/blueprint/sandbox/windows.md`「装到哪了那份记录」、「怎么走」第 4 条第 7 步）。

use miyu_kernel::time::Timestamp;

use super::*;
use crate::install::testkit::Dir;

/// 一份记录，时刻是固定的。
fn record() -> Record {
    Record::new(
        "miyu-sandbox".into(),
        "S-1-5-21-2606943370-4158592556-3158051839-1002".into(),
        "AQAAANCMnd8BFdERjHoAwE".into(),
        Timestamp::from_unix_millis(1_790_596_800_000).expect("在范围里"),
    )
}

/// 记录落在数据根的哪里。
fn file(dir: &Dir) -> std::path::PathBuf {
    dir.path().join("state").join("sandbox").join(FILE)
}

#[test]
fn a_record_is_one_line_and_reads_back() {
    let line = record().to_line().expect("写得成");
    assert_eq!(
        line,
        concat!(
            r#"{"version":1,"username":"miyu-sandbox","#,
            r#""user_sid":"S-1-5-21-2606943370-4158592556-3158051839-1002","#,
            r#""password":"AQAAANCMnd8BFdERjHoAwE","created_at":"2026-09-28T12:00:00.000Z"}"#,
            "\n"
        )
    );
    assert_eq!(Record::from_json(&line).expect("读得回来"), record());
}

#[test]
fn other_versions_extra_fields_and_missing_fields_are_not_read() {
    let line = record().to_line().expect("写得成");
    for bad in [
        line.replace(r#""version":1"#, r#""version":2"#),
        line.replace(r#""version":1"#, r#""version":0"#),
        line.replace(r#""username""#, r#""later":true,"username""#),
        line.replace(r#","created_at":"2026-09-28T12:00:00.000Z""#, ""),
        line.replace(r#""password":"AQAAANCMnd8BFdERjHoAwE""#, r#""password":7"#),
        "not json".to_string(),
        String::new(),
    ] {
        assert!(Record::from_json(&bad).is_err(), "{bad}");
    }
}

#[test]
fn it_is_written_into_the_given_data_root() {
    // 提升过的自己可能是另一个管理员账号：写到参数给的数据根里，不看自己的家目录（施工 5-8 审过时加）。
    let dir = Dir::root();
    write(&dir.owner(), &record()).expect("写得进");
    let written = std::fs::read_to_string(file(&dir)).expect("在参数给的数据根里");
    assert_eq!(written, record().to_line().expect("写得成"));
    let left: Vec<_> = std::fs::read_dir(file(&dir).parent().expect("有上一级"))
        .expect("读得了目录")
        .map(|entry| entry.expect("读得了").file_name())
        .collect();
    assert_eq!(left, [std::ffi::OsString::from(FILE)], "临时文件没留下");
}

#[test]
fn a_directory_that_is_not_a_data_root_gets_nothing() {
    let dir = Dir::new();
    let error = write(&dir.owner(), &record()).expect_err("没有标记");
    assert_eq!(
        error,
        InstallError::NotDataRoot {
            path: dir.path().display().to_string()
        }
    );
    assert!(!dir.path().join("state").exists(), "一个字节都没写");
}

#[test]
fn a_leftover_temporary_file_is_cleared_first() {
    let dir = Dir::root();
    let sandbox = dir.path().join("state").join("sandbox");
    std::fs::create_dir_all(&sandbox).expect("建得了");
    std::fs::write(sandbox.join(format!("{FILE}.tmp")), "half of an old record").expect("写得进");
    write(&dir.owner(), &record()).expect("写得进");
    assert_eq!(
        std::fs::read_to_string(file(&dir)).expect("写了"),
        record().to_line().expect("写得成")
    );
    assert!(!sandbox.join(format!("{FILE}.tmp")).exists());
}

#[test]
fn an_old_record_is_replaced() {
    let dir = Dir::root();
    write(&dir.owner(), &record()).expect("写得进");
    let mut newer = record();
    newer.password = "BBBB".into();
    write(&dir.owner(), &newer).expect("盖得掉");
    assert_eq!(
        std::fs::read_to_string(file(&dir)).expect("写了"),
        newer.to_line().expect("写得成")
    );
}

#[test]
fn deleting_removes_it_and_nothing_there_is_fine() {
    let dir = Dir::root();
    delete(&dir.owner()).expect("本来就没有，不算错");
    write(&dir.owner(), &record()).expect("写得进");
    delete(&dir.owner()).expect("删得掉");
    assert!(!file(&dir).exists());
    delete(&dir.owner()).expect("再删也不算错");
}

#[test]
fn deleting_needs_a_data_root_too() {
    let dir = Dir::new();
    assert!(matches!(
        delete(&dir.owner()),
        Err(InstallError::NotDataRoot { .. })
    ));
}
