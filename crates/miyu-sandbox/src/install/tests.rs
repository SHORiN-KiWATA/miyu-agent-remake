//! 装沙盒对外的几样（`docs/blueprint/sandbox/windows.md`「对外的样子」）：用户名的上限、SID 的写法、替谁装。

use std::path::PathBuf;

use super::*;

#[test]
fn the_user_name_fits_a_local_account() {
    // 本机用户名最多 20 个字符，不许有空格。
    assert_eq!(USER, "miyu-sandbox");
    assert!(USER.chars().count() <= 20);
    assert!(!USER.contains(' '));
}

#[test]
fn only_windows_needs_the_setup() {
    assert_eq!(needed(), cfg!(windows));
}

#[test]
fn real_sids_are_accepted() {
    for sid in [
        "S-1-5-21-2606943370-4158592556-3158051839-1001",
        "S-1-5-18",
        "S-1-1-0",
        // 一段标识机构加十五段子机构：最长的写法。
        "S-1-5-1-2-3-4-5-6-7-8-9-10-11-12-13-14-15",
    ] {
        assert!(valid_sid(sid), "{sid}");
    }
}

#[test]
fn anything_that_could_bend_the_access_rules_is_refused() {
    // SID 要原样拼进访问控制的写法里（`D:P(A;;FA;;;<SID>)…`）：多一个括号、分号，就能多给别人一条权限。
    for sid in [
        "S-1-5-21-1)(A;;FA;;;WD",
        "S-1-5-21-1;WD",
        "S-1-5-21-1 ",
        " S-1-5-21-1",
        "S-1-5-21-1\n",
        "WD",
        "SY",
        "",
        "S-1-",
        "S-1-5",
        "S-1-5-",
        "S-1-5--21",
        "S-2-5-21",
        "s-1-5-21",
        "S-1-5-21-x",
        "S-1-5-21-1-2-3-4-5-6-7-8-9-10-11-12-13-14-15",
        "S-1-５-21",
    ] {
        assert!(!valid_sid(sid), "{sid:?}");
    }
}

#[test]
fn an_owner_needs_an_absolute_data_root_and_a_real_sid() {
    let home = std::env::temp_dir().join("miyu-owner");
    let owner = Owner::new(home.clone(), "S-1-5-21-1-2-3-1001".into()).expect("收");
    assert_eq!(owner.home(), home.as_path());
    assert_eq!(owner.sid(), "S-1-5-21-1-2-3-1001");

    let relative = Owner::new(PathBuf::from("relative/home"), "S-1-5-21-1-2-3-1001".into());
    assert!(
        matches!(relative, Err(InstallError::Failed { ref step, .. }) if step == "check owner"),
        "{relative:?}"
    );
    let bent = Owner::new(home, "S-1-5-21-1)(A;;FA;;;WD".into());
    assert!(
        matches!(bent, Err(InstallError::Failed { ref step, .. }) if step == "check owner"),
        "{bent:?}"
    );
}

#[test]
fn each_error_reads_in_one_english_line() {
    assert_eq!(
        InstallError::Administrator.to_string(),
        "an administrator account named miyu-sandbox already exists"
    );
    assert_eq!(
        InstallError::NotDataRoot {
            path: "C:\\x".into()
        }
        .to_string(),
        "C:\\x is not a Miyu data root"
    );
    assert_eq!(
        InstallError::failed("create user", "access denied").to_string(),
        "create user: access denied"
    );
}
