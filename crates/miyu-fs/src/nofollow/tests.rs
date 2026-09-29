//! 路上一层链接都不跟地打开（施工 5-10 下）：普通的打得开；路上、最后一层有链接的报 `ELOOP`；一层一层打开那条路
//! （Linux 上 `openat2` 用不了时退回的，别的 Unix 上一直走的）单独测。

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 一个用完就删的临时目录，换成了真实的位置：`real/f.txt`，`via` 指向 `real`。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-nofollow-{}-{n}", std::process::id()));
        fs::create_dir_all(dir.join("real")).unwrap();
        fs::write(dir.join("real/f.txt"), "f").unwrap();
        std::os::unix::fs::symlink(dir.join("real"), dir.join("via")).unwrap();
        Scratch(fs::canonicalize(dir).unwrap())
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

fn is_loop(error: &io::Error) -> bool {
    error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
}

#[test]
fn a_plain_path_opens() {
    let scratch = Scratch::new();
    let mut text = String::new();
    open_read(&scratch.0.join("real/f.txt"))
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "f");
    assert!(open_dir(&scratch.0.join("real")).is_ok());
}

#[test]
fn a_link_on_the_way_or_at_the_end_is_refused() {
    let scratch = Scratch::new();
    let error = open_read(&scratch.0.join("via/f.txt")).unwrap_err();
    assert!(is_loop(&error), "{error}");
    let error = open_dir(&scratch.0.join("via")).unwrap_err();
    assert!(is_loop(&error), "{error}");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn opening_layer_by_layer_refuses_links_too() {
    let scratch = Scratch::new();
    let read = OFlags::RDONLY | OFlags::NONBLOCK;
    let mut text = String::new();
    File::from(by_components(&scratch.0.join("real/f.txt"), read).unwrap())
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "f");
    let error = by_components(&scratch.0.join("via/f.txt"), read).unwrap_err();
    assert!(is_loop(&error), "{error}");
    let error = by_components(&scratch.0.join("via"), dir_flags()).unwrap_err();
    assert!(is_loop(&error), "{error}");
    let error = by_components(Path::new("real/f.txt"), read).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "相对的不收");
    let error = by_components(&scratch.0.join("real/f.txt"), dir_flags()).unwrap_err();
    assert_eq!(
        error.raw_os_error(),
        Some(rustix::io::Errno::NOTDIR.raw_os_error()),
        "要的是目录，开出来是文件：{error}"
    );
}
