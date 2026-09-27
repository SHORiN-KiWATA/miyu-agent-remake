//! 整体换成新的内容：新建、覆盖；只读的不写；盖不上去的，临时文件删掉，原来的没动。

use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-replace-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    /// 目录里有些什么，照名字排。
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_file_is_created_and_then_replaced_whole() {
    let scratch = Scratch::new();
    let file = scratch.0.join("a.txt");
    replace(&file, b"one").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"one");
    replace(&file, b"two").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"two");
    assert_eq!(scratch.names(), ["a.txt"], "没留下临时文件");
}

#[test]
fn a_read_only_file_is_not_replaced() {
    let scratch = Scratch::new();
    let file = scratch.0.join("locked.txt");
    fs::write(&file, "keep").unwrap();
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&file, permissions).unwrap();
    let error = replace(&file, b"new").unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read(&file).unwrap(), b"keep");
    assert_eq!(scratch.names(), ["locked.txt"]);
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "测试收尾：放开只读好删掉目录"
    )]
    permissions.set_readonly(false);
    fs::set_permissions(&file, permissions).unwrap();
}

#[test]
fn when_it_cannot_be_put_in_place_the_temporary_file_goes_away() {
    // 要盖的是一个不空的目录：临时文件写好了，改名盖不上去。
    let scratch = Scratch::new();
    let dir = scratch.0.join("dir");
    fs::create_dir_all(dir.join("inside")).unwrap();
    assert!(replace(&dir, b"x").is_err());
    assert_eq!(scratch.names(), ["dir"], "临时文件删掉了");
    assert!(dir.join("inside").is_dir(), "原来的没动");
}
