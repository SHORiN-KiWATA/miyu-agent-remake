//! 一个效果写成的一项：移进回收站的只写路径和 `trash`，读文件、派任务的不算。

use miyu_kernel::event::{FileRead, FileTrashed};
use miyu_kernel::id::ContentHash;

use super::*;

#[test]
fn a_trashed_file_has_no_diff_and_a_read_is_not_a_file() {
    let blobs = Blobs::new(std::env::temp_dir().join("miyu-detail-unused"));
    let trashed = Effect::FileTrashed(FileTrashed {
        path: "/home/me/a.txt".to_string(),
        trash: "/home/me/.local/share/Trash/files/a.txt".to_string(),
    });
    assert_eq!(
        file(&blobs, &trashed),
        Some(json!({"path": "/home/me/a.txt", "action": "trash"}))
    );
    let read = Effect::FileRead(FileRead {
        path: "/home/me/a.txt".to_string(),
        lines: None,
        hash: ContentHash::of(b"a\n"),
    });
    assert_eq!(file(&blobs, &read), None);
}
