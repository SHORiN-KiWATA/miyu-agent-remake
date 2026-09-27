//! 系统的回收站（施工 4-7 上）：放进去再移回来。Linux 上回收站在场地的假家目录里，移回来以后 `.trashinfo` 删了，
//! 上级目录没了的建上；macOS、Windows 上放进系统真的回收站再移回来（在 CI 上跑）。

mod support;

use std::fs;
use std::path::Path;

use miyu_fs::trash::{put, restore};

use support::Scratch;

/// 把 `real` 放进回收站，交回它在回收站里的位置。
fn kept(real: &Path, home: Option<&Path>) -> String {
    put(real, home).unwrap_or_else(|refused| panic!("{}: {refused:?}", real.display()))
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use support::file;

    /// 场地里的假家目录，交给回收站的就是它：不会碰到真的回收站。
    fn home(scratch: &Scratch) -> std::path::PathBuf {
        let home = scratch.0.join("home");
        fs::create_dir_all(&home).expect("建得了");
        home
    }

    #[test]
    fn a_file_comes_back_and_its_record_goes() {
        let scratch = Scratch::new();
        let home = home(&scratch);
        file(&scratch.0.join("work/a.txt"), "old\n");
        let real = fs::canonicalize(scratch.0.join("work/a.txt")).expect("在");
        let kept = kept(&real, Some(&home));
        let record = home.join(".local/share/Trash/info/a.txt.trashinfo");
        assert!(record.is_file(), "放进去时记下了");
        restore(Path::new(&kept), &real).expect("移得回来");
        assert_eq!(fs::read_to_string(&real).expect("回来了"), "old\n");
        assert!(!Path::new(&kept).exists(), "回收站里没有了");
        assert!(!record.exists(), "记录删了");
    }

    #[test]
    fn a_directory_comes_back_even_when_its_parent_is_gone() {
        let scratch = Scratch::new();
        let home = home(&scratch);
        file(&scratch.0.join("work/deep/dir/inner.txt"), "in\n");
        let real = fs::canonicalize(scratch.0.join("work/deep/dir")).expect("在");
        let kept = kept(&real, Some(&home));
        fs::remove_dir(scratch.0.join("work/deep")).expect("空了删得掉");
        restore(Path::new(&kept), &real).expect("移得回来");
        assert_eq!(
            fs::read_to_string(real.join("inner.txt")).expect("回来了"),
            "in\n"
        );
        assert!(!home.join(".local/share/Trash/info/dir.trashinfo").exists());
    }

    /// 移回来的不在回收站的 `files/` 下：没有它的记录，旁边同名的 `.trashinfo` 不动。
    #[test]
    fn only_a_record_of_the_trash_is_removed() {
        let scratch = Scratch::new();
        file(&scratch.0.join("elsewhere/x.txt"), "x");
        file(&scratch.0.join("info/x.txt.trashinfo"), "[Trash Info]\n");
        let to = scratch.0.join("work/x.txt");
        restore(&scratch.0.join("elsewhere/x.txt"), &to).expect("移得回来");
        assert_eq!(fs::read_to_string(&to).expect("回来了"), "x");
        assert!(
            scratch.0.join("info/x.txt.trashinfo").is_file(),
            "别人的不动"
        );
    }

    #[test]
    fn what_is_no_longer_in_the_trash_cannot_come_back() {
        let scratch = Scratch::new();
        let to = scratch.0.join("work/gone.txt");
        assert!(restore(&scratch.0.join("nowhere/gone.txt"), &to).is_err());
        assert!(!to.exists());
    }
}

#[cfg(any(target_os = "macos", windows))]
#[test]
fn files_and_directories_come_back_from_the_system_trash() {
    let scratch = Scratch::new();
    support::file(&scratch.0.join("work/a.txt"), "old\n");
    support::file(&scratch.0.join("work/dir/inner.txt"), "in\n");
    for name in ["a.txt", "dir"] {
        let real = fs::canonicalize(scratch.0.join("work").join(name)).expect("在");
        let kept = kept(&real, None);
        assert!(!real.exists(), "{name} 进了回收站");
        restore(Path::new(&kept), &real).expect("移得回来");
        assert!(real.exists(), "{name} 回来了");
        assert!(!Path::new(&kept).exists(), "{name}：回收站里没有了");
        // Windows 上 `$R` 旁边那份 `$I` 记录也删了。
        #[cfg(windows)]
        {
            let kept = Path::new(&kept);
            let rest = kept
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("$R"))
                .expect("是 $R 开头的");
            assert!(!kept.with_file_name(format!("$I{rest}")).exists());
        }
    }
    assert_eq!(
        fs::read_to_string(scratch.0.join("work/a.txt")).expect("回来了"),
        "old\n"
    );
}
