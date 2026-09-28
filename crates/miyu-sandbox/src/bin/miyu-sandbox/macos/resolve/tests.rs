use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 一个用完就删的临时目录，里面有目录 `real` 和指到它的链接 `link`；`real` 字段是这个临时目录换成真实的位置以后
/// （macOS 上临时目录在 `/private` 下的链接后面）。
struct Dir {
    path: PathBuf,
    real: PathBuf,
}

impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("miyu-sandbox-resolve-{}-{n}", std::process::id()));
        std::fs::create_dir_all(path.join("real")).expect("建得了目录");
        symlink("real", path.join("link")).expect("建得了链接");
        let real = std::fs::canonicalize(&path).expect("换得了");
        Dir { path, real }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.path) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

/// 规格照 JSON 读：规格的格以后怎么增减，这里都不用改。
fn spec(write: &[&Path], hidden: &[&Path]) -> Spec {
    let json = serde_json::json!({ "write": write, "hidden": hidden });
    Spec::from_json(&json.to_string()).expect("读得懂")
}

#[test]
fn links_become_their_real_places() {
    let dir = Dir::new();
    let given = dir.path.join("link/file");
    let real = dir.real.join("real/file");
    let resolved = resolve(&spec(&[&given], &[&given])).expect("换得了");
    // 能写的只用换过的；藏起来的原样也留着：挡住链接本身。
    assert_eq!(resolved.write, std::slice::from_ref(&real));
    assert_eq!(resolved.hidden, [given, real]);
}

#[test]
fn missing_paths_go_through_the_nearest_existing_parent() {
    let dir = Dir::new();
    let given = dir.path.join("link/not-yet/deeper");
    let resolved = resolve(&spec(&[&given], &[])).expect("换得了");
    assert_eq!(resolved.write, [dir.real.join("real/not-yet/deeper")]);
}

#[test]
fn paths_already_real_and_repeated_are_written_once() {
    let dir = Dir::new();
    let real = dir.real.join("real");
    let resolved = resolve(&spec(&[&real, &real], &[&real, &real])).expect("换得了");
    assert_eq!(resolved.write, std::slice::from_ref(&real));
    assert_eq!(resolved.hidden, [real]);
}

#[test]
fn extra_slashes_are_tidied() {
    let dir = Dir::new();
    let untidy = PathBuf::from(format!("{}//real/", dir.real.display()));
    let resolved = resolve(&spec(&[], &[&untidy])).expect("换得了");
    assert_eq!(resolved.hidden, [dir.real.join("real")]);
}

#[test]
fn relative_dotted_and_nul_paths_cannot_be_confined() {
    let cases = [
        ("relative/x", r#"path is not absolute: "relative/x""#),
        ("/a/../b", r#"path has . or ..: "/a/../b""#),
        ("/a/./b", r#"path has . or ..: "/a/./b""#),
        ("/a/..", r#"path has . or ..: "/a/..""#),
        ("/a\0b", r#"path has a NUL byte: "/a\0b""#),
    ];
    for (given, said) in cases {
        let path = Path::new(given);
        let as_write = resolve(&spec(&[path], &[])).expect_err(given);
        assert_eq!(as_write, said, "能写的：{given}");
        let as_hidden = resolve(&spec(&[], &[path])).expect_err(given);
        assert_eq!(as_hidden, said, "藏起来的：{given}");
    }
}
