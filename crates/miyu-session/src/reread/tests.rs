//! 压完重读的测试（施工 6-5）：读到了存成 blob；太大的、没有的、不是 UTF-8 的、目录、链接；照先后一个一项。

use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::id::ContentHash;

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-reread-{}-{n}", std::process::id()));
        std::fs::create_dir_all(dir.join("blobs")).expect("建得了");
        // 换成真实的位置：交给重读的是效果里的路径，都是真实的位置；macOS 的临时目录在 `/var` 这条链接下面，安全打开
        // 不走任何一层链接。
        Scratch(std::fs::canonicalize(dir).expect("在"))
    }

    fn file(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).expect("写得进");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    #[expect(clippy::let_underscore_must_use, reason = "删不掉就留在临时目录里")]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn each_file_comes_back_in_order() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.join("blobs"));
    let small = scratch.file("a.rs", b"fn a() {}\n");
    let big = scratch.file("big.txt", &[b'x'; 21]);
    let exact = scratch.file("exact.txt", &[b'y'; 20]);
    let binary = scratch.file("app.bin", &[0xff, 0xfe, 0x00]);
    let dir = scratch.0.join("sub");
    std::fs::create_dir(&dir).expect("建得了");
    let missing = scratch.0.join("gone.txt").to_string_lossy().into_owned();
    let paths = [
        small,
        big,
        exact,
        binary,
        dir.to_string_lossy().into_owned(),
        missing,
    ];
    let got = reread(&paths, 20, &blobs);
    assert_eq!(got.len(), 6);
    let Reread::Read { blob, text } = &got[0] else {
        panic!("{:?}", got[0]);
    };
    assert_eq!(text, "fn a() {}\n");
    assert_eq!(*blob, ContentHash::of(b"fn a() {}\n"));
    assert_eq!(blobs.get(blob).expect("存进了 blob"), b"fn a() {}\n");
    assert_eq!(got[1], Reread::TooLarge, "超过上限一个字节");
    assert!(
        matches!(&got[2], Reread::Read { text, .. } if text.len() == 20),
        "正好上限的读"
    );
    assert_eq!(got[3], Reread::Unreadable, "不是 UTF-8");
    assert_eq!(got[4], Reread::Unreadable, "目录");
    assert_eq!(got[5], Reread::Unreadable, "没有");
}

#[cfg(unix)]
#[test]
fn a_link_is_not_followed() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.join("blobs"));
    let target = scratch.file("real.txt", b"secret\n");
    let link = scratch.0.join("link.txt");
    std::os::unix::fs::symlink(&target, &link).expect("建得了链接");
    let got = reread(&[link.to_string_lossy().into_owned()], 100, &blobs);
    assert_eq!(got, [Reread::Unreadable]);
}
