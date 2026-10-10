//! 取到手、存下来（施工 O-33）：照 url、路径、base64 的先后拿字节，前面的不行换后面的，都不行说每一样为什么；文件名洗掉路径
//! 分隔符、开头的点；只写进工作区的 `qq-files/`，先写临时文件再改名，同一个文件取两次换成新的；`qq-files` 是链接的、指到外面的
//! 不写，文件名那里本来是链接的换掉、不写进它指的地方；`~` 换成家目录。

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;

/// 测试用的临时目录：放下时删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let dir = std::env::temp_dir().join(format!("miyu-onebot-media-{}", unique()));
        std::fs::create_dir_all(&dir).expect("建得了");
        Scratch(std::fs::canonicalize(&dir).expect("有真实位置"))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.0).is_err() {
            // 删不掉的留在临时目录里。
        }
    }
}

/// 在 `at` 建一个指向目录 `target` 的链接；这台机器建不了的（Windows 没开开发者模式）交回假。
fn link_dir(target: &Path, at: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, at);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(target, at);
    made.is_ok()
}

/// 同 [`link_dir`]，指向文件。
fn link_file(target: &Path, at: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, at);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(target, at);
    made.is_ok()
}

#[test]
fn names_lose_separators_and_leading_dots() {
    assert_eq!(file_name("8815", 1, Some("排班.pdf")), "8815-1-排班.pdf");
    assert_eq!(file_name("8815", 2, None), "8815-2");
    assert_eq!(
        file_name("8815", 1, Some("../../etc/passwd")),
        "8815-1-etcpasswd"
    );
    assert_eq!(file_name("8815", 1, Some("..\\..\\a.txt")), "8815-1-a.txt");
    assert_eq!(file_name("8815", 1, Some(".bashrc")), "8815-1-bashrc");
    assert_eq!(file_name("8815", 1, Some(" . .hidden. ")), "8815-1-hidden");
    assert_eq!(
        file_name("8815", 1, Some("a\u{0}b\nc:d*e?f\"g<h>i|j")),
        "8815-1-abcdefghij"
    );
    assert_eq!(file_name("8815", 1, Some("...")), "8815-1");
    assert_eq!(file_name("8815", 1, Some("/")), "8815-1");
    let long = "长".repeat(300);
    assert_eq!(
        file_name("8815", 1, Some(&long)),
        format!("8815-1-{}", "长".repeat(NAME))
    );
}

#[tokio::test]
async fn bytes_come_from_the_first_source_that_works() {
    let scratch = Scratch::new();
    let file = scratch.path().join("napcat.bin");
    std::fs::write(&file, b"from path").expect("写得进");
    let missing = scratch.path().join("missing.bin");
    let wait = Duration::from_secs(5);
    // 地址连不上（本机回环上没人听的端口），换路径。
    let sources = [
        Source::Url("http://127.0.0.1:9/a".to_string()),
        Source::Path(missing.clone()),
        Source::Path(file),
        Source::Base64("ZnJvbSBiYXNlNjQ=".to_string()),
    ];
    assert_eq!(bytes(&sources, wait).await.unwrap(), b"from path");
    assert_eq!(
        bytes(&[Source::Base64("ZnJvbSBiYXNlNjQ=".to_string())], wait)
            .await
            .unwrap(),
        b"from base64"
    );
    let failed = bytes(
        &[
            Source::Path(missing),
            Source::Path(scratch.path().to_path_buf()),
            Source::Base64("!!".to_string()),
        ],
        wait,
    )
    .await
    .unwrap_err();
    assert_eq!(failed.matches("path:").count(), 2, "{failed}");
    assert!(failed.ends_with("base64: not valid"), "{failed}");
    assert_eq!(
        bytes(&[], wait).await.unwrap_err(),
        "no url, path or base64 in the answer"
    );
}

#[test]
fn a_file_lands_in_qq_files_and_a_second_fetch_replaces_it() {
    let scratch = Scratch::new();
    let workspace = scratch.path().to_string_lossy().into_owned();
    let saved = save(&workspace, None, "8815-1-a.txt", b"one").expect("存得下");
    assert_eq!(saved, scratch.path().join(QQ_FILES).join("8815-1-a.txt"));
    assert_eq!(std::fs::read(&saved).unwrap(), b"one");
    let again = save(&workspace, None, "8815-1-a.txt", b"two").expect("存得下");
    assert_eq!(again, saved);
    assert_eq!(std::fs::read(&saved).unwrap(), b"two");
    let left: Vec<_> = std::fs::read_dir(scratch.path().join(QQ_FILES))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(left, ["8815-1-a.txt"], "临时文件都改了名");
}

#[test]
fn the_workspace_may_start_with_a_tilde() {
    let scratch = Scratch::new();
    std::fs::create_dir(scratch.path().join("ws")).unwrap();
    let saved = save("~/ws", Some(scratch.path()), "1-1", b"x").expect("存得下");
    assert_eq!(saved, scratch.path().join("ws").join(QQ_FILES).join("1-1"));
    assert!(save("~", None, "1-1", b"x").unwrap_err().contains("home"));
    assert!(save("~bob/ws", Some(scratch.path()), "1-1", b"x").is_err());
    assert!(save("relative/ws", None, "1-1", b"x").is_err());
    let gone = scratch.path().join("gone").to_string_lossy().into_owned();
    assert!(save(&gone, None, "1-1", b"x").is_err(), "工作区没有");
}

#[test]
fn links_lead_nowhere_outside() {
    let scratch = Scratch::new();
    let workspace = scratch.path().join("ws");
    let outside = scratch.path().join("outside");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let cwd = workspace.to_string_lossy().into_owned();
    if !link_dir(&outside, &workspace.join(QQ_FILES)) {
        return;
    }
    let refused = save(&cwd, None, "1-1-a.txt", b"x").unwrap_err();
    assert!(refused.contains("not a folder"), "{refused}");
    assert_eq!(
        std::fs::read_dir(&outside).unwrap().count(),
        0,
        "外面什么都没写"
    );
    // `qq-files` 是真目录、文件名那里是指到外面的链接：换掉链接本身，外面的文件不动。
    // 指向目录的链接：Unix 上照文件删，Windows 上要照目录删（`remove_file` 回「拒绝访问」，CI 的 Windows 撞出来的）。
    let link = workspace.join(QQ_FILES);
    std::fs::remove_file(&link)
        .or_else(|_| std::fs::remove_dir(&link))
        .unwrap();
    std::fs::create_dir(workspace.join(QQ_FILES)).unwrap();
    let secret = outside.join("secret.txt");
    std::fs::write(&secret, b"keep").unwrap();
    if !link_file(&secret, &workspace.join(QQ_FILES).join("1-1-a.txt")) {
        return;
    }
    let saved = save(&cwd, None, "1-1-a.txt", b"new").expect("存得下");
    assert_eq!(std::fs::read(&secret).unwrap(), b"keep");
    assert_eq!(std::fs::read(&saved).unwrap(), b"new");
    assert!(
        !std::fs::symlink_metadata(&saved)
            .unwrap()
            .file_type()
            .is_symlink(),
        "换成了普通文件"
    );
}

#[test]
fn a_name_that_is_not_plain_is_refused() {
    let scratch = Scratch::new();
    let cwd = scratch.path().to_string_lossy().into_owned();
    for name in ["../x", "..", "a/b"] {
        assert!(save(&cwd, None, name, b"x").is_err(), "{name}");
    }
    assert_eq!(
        std::fs::read_dir(scratch.path()).unwrap().count(),
        1,
        "只建了 qq-files"
    );
}
