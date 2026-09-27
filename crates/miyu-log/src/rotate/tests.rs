//! 按大小轮换：满了换一份，一行不拆开，正在写的之外留几份，再起来接着写。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-log-test-{}-{n}", std::process::id())))
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

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn it_rotates_between_lines_and_keeps_only_so_many() {
    let scratch = Scratch::new();
    // 每行 9 个字节加换行 10 个；一份最多 20：两行正好写满，不换，第三行才换。
    let file = RotatingFile::open(&scratch.0, "core", 20, 3).unwrap();
    for n in 0..9 {
        file.write_line(&format!("line {n:04}"));
    }
    let dir = &scratch.0;
    assert_eq!(read(&dir.join("core.log")), "line 0008\n");
    assert_eq!(read(&dir.join("core.log.1")), "line 0006\nline 0007\n");
    assert_eq!(read(&dir.join("core.log.2")), "line 0004\nline 0005\n");
    assert_eq!(read(&dir.join("core.log.3")), "line 0002\nline 0003\n");
    // 最老的两行删掉了：正在写的之外只留 3 份。
    assert!(!dir.join("core.log.4").exists());
    // 一行不拆开：每一份都是整行。
    for name in ["core.log", "core.log.1", "core.log.2", "core.log.3"] {
        assert!(read(&dir.join(name)).ends_with('\n'));
    }
}

#[test]
fn a_line_longer_than_the_limit_still_goes_in_whole() {
    let scratch = Scratch::new();
    let file = RotatingFile::open(&scratch.0, "core", 5, 3).unwrap();
    file.write_line("a line far longer than five bytes");
    assert_eq!(read(&file.path()), "a line far longer than five bytes\n");
    // 空的那一份不换：不会留下一份空的 `.1`。
    assert!(!scratch.0.join("core.log.1").exists());
}

#[test]
fn a_restart_goes_on_writing_the_same_file_and_counts_what_is_there() {
    let scratch = Scratch::new();
    {
        let file = RotatingFile::open(&scratch.0, "core", 25, 3).unwrap();
        file.write_line("line 0000");
        file.flush();
    }
    let file = RotatingFile::open(&scratch.0, "core", 25, 3).unwrap();
    file.write_line("line 0001");
    assert_eq!(read(&file.path()), "line 0000\nline 0001\n");
    drop(file);
    // 再起来时量了原来有多大：已经 20 个字节，第三行放不下，换一份。
    let file = RotatingFile::open(&scratch.0, "core", 25, 3).unwrap();
    file.write_line("line 0002");
    assert_eq!(read(&file.path()), "line 0002\n");
    assert_eq!(
        read(&scratch.0.join("core.log.1")),
        "line 0000\nline 0001\n"
    );
}
