//! 提升过的自己没成时写下的原因（`docs/blueprint/sandbox/windows.md`「对外的样子」、「怎么走」第 3、6 条）。

use super::*;
use crate::install::testkit::Dir;

/// 原因落在数据根的哪里。
fn file(dir: &Dir) -> std::path::PathBuf {
    dir.path().join("state").join("sandbox").join(FILE)
}

#[test]
fn each_reason_is_one_line_and_reads_back() {
    let cases = [
        (InstallError::Administrator, r#"{"error":"administrator"}"#),
        (
            InstallError::NotDataRoot {
                path: r"C:\Users\me\somewhere".into(),
            },
            r#"{"error":"not-data-root","path":"C:\\Users\\me\\somewhere"}"#,
        ),
        (
            InstallError::failed("create user", "Access is denied. (os error 5)"),
            r#"{"error":"failed","step":"create user","detail":"Access is denied. (os error 5)"}"#,
        ),
    ];
    for (error, line) in cases {
        let dir = Dir::root();
        let owner = dir.owner();
        report(&owner, &error).expect("写得进");
        assert_eq!(
            std::fs::read_to_string(file(&dir)).expect("写在参数给的数据根里"),
            format!("{line}\n")
        );
        assert_eq!(reported(&owner).expect("读得懂"), Some(error));
    }
}

#[test]
fn nothing_reported_is_nothing() {
    let dir = Dir::root();
    assert_eq!(reported(&dir.owner()).expect("没有不算错"), None);
}

#[test]
fn clearing_removes_it_and_nothing_there_is_fine() {
    let dir = Dir::root();
    let owner = dir.owner();
    clear_report(&owner).expect("本来就没有，不算错");
    report(&owner, &InstallError::Administrator).expect("写得进");
    clear_report(&owner).expect("删得掉");
    assert!(!file(&dir).exists());
    assert_eq!(reported(&owner).expect("清掉了"), None);
}

#[test]
fn a_second_report_replaces_the_first() {
    let dir = Dir::root();
    let owner = dir.owner();
    report(&owner, &InstallError::Administrator).expect("写得进");
    let later = InstallError::failed("hide user", "gone");
    report(&owner, &later).expect("盖得掉");
    assert_eq!(reported(&owner).expect("读得懂"), Some(later));
}

#[test]
fn a_report_that_cannot_be_understood_says_so() {
    let dir = Dir::root();
    let owner = dir.owner();
    std::fs::create_dir_all(file(&dir).parent().expect("有上一级")).expect("建得了");
    std::fs::write(file(&dir), "{\"error\":\"on fire\"}\n").expect("写得进");
    let error = reported(&owner).expect_err("读不懂");
    assert!(
        matches!(error, InstallError::Failed { ref step, .. } if step == "read report"),
        "{error:?}"
    );
}

#[test]
fn a_report_needs_a_data_root() {
    let dir = Dir::new();
    assert!(matches!(
        report(&dir.owner(), &InstallError::Administrator),
        Err(InstallError::NotDataRoot { .. })
    ));
}
