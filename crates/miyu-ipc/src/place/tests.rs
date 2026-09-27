use std::path::{Path, PathBuf};

use super::*;
use crate::test_support::root_at;

fn dirs(platform: Platform, runtime_dir: Option<&str>) -> Dirs {
    Dirs {
        platform,
        runtime_dir: runtime_dir.map(PathBuf::from),
        temp_dir: PathBuf::from("/tmp"),
        uid: Some(1000),
    }
}

/// 在 `/d/` 下、`run/core.sock` 正好 `bytes` 个字节的数据根。
fn root_of_socket_length(bytes: usize) -> DataRoot {
    let tail = "/run/core.sock".len();
    let name = "a".repeat(bytes - tail - "/d/".len());
    root_at(&Path::new("/d").join(name))
}

#[test]
fn the_fingerprint_is_the_first_eight_hex_of_the_path_hash() {
    // printf '%s' /data/miyu | sha256sum
    assert_eq!(fingerprint(&root_at(Path::new("/data/miyu"))), "df81e9ca");
    assert_eq!(fingerprint(&root_at(Path::new("/data/miyu2"))), "4b3e57cb");
}

#[test]
fn linux_puts_it_under_the_runtime_dir() {
    let root = root_at(Path::new("/data/miyu"));
    let path = locate(&root, &dirs(Platform::Linux, Some("/run/user/1000"))).expect("放得下");
    assert_eq!(path, Path::new("/run/user/1000/miyu-df81e9ca/core.sock"));
}

#[test]
fn without_a_runtime_dir_it_goes_into_run() {
    let root = root_at(Path::new("/data/miyu"));
    for platform in [Platform::Linux, Platform::Macos] {
        let path = locate(&root, &dirs(platform, None)).expect("放得下");
        assert_eq!(path, Path::new("/data/miyu/run/core.sock"));
    }
}

#[test]
fn macos_ignores_the_runtime_dir() {
    let root = root_at(Path::new("/data/miyu"));
    let path = locate(&root, &dirs(Platform::Macos, Some("/run/user/501"))).expect("放得下");
    assert_eq!(path, Path::new("/data/miyu/run/core.sock"));
}

#[test]
fn too_long_goes_into_the_temp_dir() {
    let root = root_at(&Path::new("/d").join("a".repeat(120)));
    let print = fingerprint(&root);
    for platform in [Platform::Linux, Platform::Macos] {
        let path = locate(&root, &dirs(platform, None)).expect("临时目录下放得下");
        assert_eq!(path, PathBuf::from(format!("/tmp/miyu-1000/{print}.sock")));
    }
    let long_runtime = format!("/run/{}", "r".repeat(120));
    let path = locate(&root, &dirs(Platform::Linux, Some(&long_runtime))).expect("放得下");
    assert_eq!(path, PathBuf::from(format!("/tmp/miyu-1000/{print}.sock")));
}

#[test]
fn the_limits_count_bytes_without_the_trailing_zero() {
    for (platform, limit) in [(Platform::Linux, 107), (Platform::Macos, 103)] {
        let at_limit = root_of_socket_length(limit);
        let path = locate(&at_limit, &dirs(platform, None)).expect("放得下");
        assert_eq!(path, at_limit.run().join("core.sock"), "{platform:?}");
        let over = root_of_socket_length(limit + 1);
        let path = locate(&over, &dirs(platform, None)).expect("临时目录下放得下");
        assert!(
            path.starts_with("/tmp/miyu-1000"),
            "{platform:?}：{}",
            path.display()
        );
    }
}

#[test]
fn nowhere_to_put_it_is_an_error() {
    let root = root_at(&Path::new("/d").join("a".repeat(120)));
    let mut long_temp = dirs(Platform::Linux, None);
    long_temp.temp_dir = PathBuf::from(format!("/t/{}", "t".repeat(120)));
    let error = locate(&root, &long_temp).expect_err("放不下");
    assert!(
        matches!(&error, OpenError::TooLong(path) if *path == root.run().join("core.sock")),
        "{error:?}"
    );
    let mut no_uid = dirs(Platform::Linux, None);
    no_uid.uid = None;
    let error = locate(&root, &no_uid).expect_err("没有用户编号，没有临时目录这条路");
    assert!(matches!(error, OpenError::TooLong(_)), "{error:?}");
}

#[test]
fn windows_has_no_socket_yet() {
    let root = root_at(Path::new("/data/miyu"));
    let error = locate(&root, &dirs(Platform::Windows, None)).expect_err("还不支持");
    assert!(matches!(error, OpenError::Unsupported), "{error:?}");
}
