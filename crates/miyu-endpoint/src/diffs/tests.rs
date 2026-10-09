//! 差异的几种算不出：不是 UTF-8 的、取不出来的、太大的；两边一样的是空的。

use super::*;

#[test]
fn what_cannot_be_diffed_says_why() {
    assert_eq!(unified(b"a\n", &[0xff, 0xfe]), Err(Skipped::Binary));
    let same = unified(b"a\n", b"a\n").expect("是文字");
    assert!(same.lines.is_empty() && same.added == 0 && same.removed == 0);
    let scratch = std::env::temp_dir().join(format!("miyu-diffs-{}", std::process::id()));
    let blobs = Blobs::new(scratch.clone());
    let absent = ContentHash::of(b"never stored");
    assert_eq!(side(&blobs, Some(&absent)), Err(Skipped::Missing));
    assert_eq!(side(&blobs, None), Ok(Vec::new()), "新建的文件的改前当空的");
    let big = vec![b'x'; usize::try_from(MOST_BYTES).unwrap_or(usize::MAX) + 1];
    let stored = blobs.put(&big).expect("存得进");
    assert_eq!(side(&blobs, Some(&stored)), Err(Skipped::TooBig));
    drop(std::fs::remove_dir_all(scratch));
    assert_eq!(Skipped::TooBig.as_str(), "too_big");
    assert_eq!(Skipped::Binary.as_str(), "binary");
    assert_eq!(Skipped::Missing.as_str(), "missing");
}
