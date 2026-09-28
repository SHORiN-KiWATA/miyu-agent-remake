//! 找助手（`docs/blueprint/sandbox.md`「怎么走」第 1 条）：主程序旁边有的找得到；没有的、是目录的找不到。

mod support;

use miyu_sandbox::{HELPER as NAME, locate};
use support::Dir;

#[test]
fn the_helper_beside_the_program_is_found() {
    let dir = Dir::new();
    let exe = dir.file("miyu", b"");
    let helper = dir.file(NAME, b"");
    assert_eq!(locate(&exe), Some(helper));
}

#[test]
fn no_helper_or_a_directory_is_not_found() {
    let dir = Dir::new();
    let exe = dir.file("miyu", b"");
    assert_eq!(locate(&exe), None, "没有");
    std::fs::create_dir(dir.path().join(NAME)).expect("建得了目录");
    assert_eq!(locate(&exe), None, "是目录");
}

#[test]
fn the_name_carries_the_windows_suffix() {
    let expected = format!("miyu-sandbox{}", std::env::consts::EXE_SUFFIX);
    assert_eq!(NAME, expected);
}
