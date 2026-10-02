//! 读一段（施工 W-6）：从 `offset` 起读 `length` 个字节、读到结尾就停；`offset` 过了结尾的是空的；`length`
//! 写 0 只问大小；没有这个文件、不是普通文件照 [`crate::open_file`] 的错。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::open::Kind;

use super::*;

/// 一个用完就删的临时目录，里面有一份 10 个字节的文件 `f.txt`：`0123456789`。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-range-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("f.txt"), b"0123456789").unwrap();
        // 换成真实的位置：macOS 上系统的临时目录在 `/var` 下，它是个链接。
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

#[test]
fn a_middle_segment_is_read() {
    let scratch = Scratch::new();
    let segment = read_range(&scratch.0.join("f.txt"), 3, 4).unwrap();
    assert_eq!(segment.data, b"3456");
    assert_eq!(segment.size, 10);
}

#[test]
fn a_length_past_the_end_stops_at_the_end() {
    let scratch = Scratch::new();
    let segment = read_range(&scratch.0.join("f.txt"), 8, 100).unwrap();
    assert_eq!(segment.data, b"89");
    assert_eq!(segment.size, 10);
}

#[test]
fn an_offset_past_the_end_is_empty() {
    let scratch = Scratch::new();
    let segment = read_range(&scratch.0.join("f.txt"), 20, 4).unwrap();
    assert_eq!(segment.data, Vec::<u8>::new());
    assert_eq!(segment.size, 10);
}

#[test]
fn an_offset_exactly_at_the_end_is_empty() {
    let scratch = Scratch::new();
    let segment = read_range(&scratch.0.join("f.txt"), 10, 4).unwrap();
    assert_eq!(segment.data, Vec::<u8>::new());
    assert_eq!(segment.size, 10);
}

#[test]
fn a_zero_length_only_reports_the_size() {
    let scratch = Scratch::new();
    let segment = read_range(&scratch.0.join("f.txt"), 0, 0).unwrap();
    assert_eq!(segment.data, Vec::<u8>::new());
    assert_eq!(segment.size, 10);
    // `offset` 不是 0 也一样：先问大小不看 `offset`。
    let segment = read_range(&scratch.0.join("f.txt"), 5, 0).unwrap();
    assert_eq!(segment.data, Vec::<u8>::new());
    assert_eq!(segment.size, 10);
}

#[test]
fn the_whole_file_is_read_when_the_length_covers_it() {
    let scratch = Scratch::new();
    let segment = read_range(&scratch.0.join("f.txt"), 0, 1024).unwrap();
    assert_eq!(segment.data, b"0123456789");
    assert_eq!(segment.size, 10);
}

#[test]
fn a_missing_file_is_not_found() {
    let scratch = Scratch::new();
    let error = read_range(&scratch.0.join("missing.txt"), 0, 4).unwrap_err();
    assert!(matches!(error, OpenError::NotFound), "{error:?}");
}

#[test]
fn a_directory_is_not_a_file() {
    let scratch = Scratch::new();
    let error = read_range(&scratch.0, 0, 4).unwrap_err();
    assert!(
        matches!(error, OpenError::NotAFile(Kind::Directory)),
        "{error:?}"
    );
}
