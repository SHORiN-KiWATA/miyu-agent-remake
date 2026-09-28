//! 叫停的旗（施工 4-9 再补一）：`write`、`edit`、`trash` 真正改之前看一眼，旗举了就不改，交回 `stopped`，什么都
//! 没动：要建的文件、上级目录还没有，要改的文件照旧，要删的还在原处。旗没举的照常改，见各自的测试。

mod support;

use serde_json::json;

use miyu_kernel::id::ContentHash;
use miyu_tool::{Done, Seen, Stop};

use support::Site;

/// 一面举起来的旗。
fn raised() -> Stop {
    let stop = Stop::default();
    stop.raise();
    stop
}

/// 她看过场地里的这个文件，看到的就是它现在的内容。
fn seen(site: &Site, path: &str) -> Seen {
    let bytes = std::fs::read(site.0.join(path)).expect("读得出");
    [(site.real(path), ContentHash::of(&bytes))]
        .into_iter()
        .collect()
}

/// 停在了改之前：没出错，字、说法、效果都没有，内容由内核写。
fn stopped(done: &Done) {
    assert!(done.stopped, "{done:?}");
    assert!(!done.error, "{done:?}");
    assert!(done.blocks.is_empty(), "{done:?}");
    assert_eq!((&done.human, done.effects.len()), (&None, 0));
}

/// 场地里 `path` 现在的内容。
fn content(site: &Site, path: &str) -> Vec<u8> {
    std::fs::read(site.0.join(path)).expect("读得出")
}

#[tokio::test]
async fn write_creates_and_overwrites_nothing_once_stopped() {
    let site = Site::new();
    let args = json!({"file_path": "new/a.txt", "content": "x"});
    stopped(
        &site
            .done_with("work", "write", args, Seen::new(), raised())
            .await,
    );
    assert!(!site.0.join("work/new").exists(), "上级目录也没建");
    site.file("work/b.txt", b"old");
    let args = json!({"file_path": "b.txt", "content": "new"});
    let seen = seen(&site, "work/b.txt");
    stopped(&site.done_with("work", "write", args, seen, raised()).await);
    assert_eq!(content(&site, "work/b.txt"), b"old");
}

#[tokio::test]
async fn edit_leaves_the_file_as_it_was_once_stopped() {
    let site = Site::new();
    site.file("work/a.rs", b"fn main() {}\n");
    let args = json!({
        "file_path": "a.rs",
        "edits": [{"old_string": "main", "new_string": "start"}],
    });
    let seen = seen(&site, "work/a.rs");
    stopped(&site.done_with("work", "edit", args, seen, raised()).await);
    assert_eq!(content(&site, "work/a.rs"), b"fn main() {}\n");
}

#[tokio::test]
async fn trash_leaves_the_file_in_place_once_stopped() {
    let site = Site::new();
    site.file("work/old.txt", b"x");
    let args = json!({"file_path": "old.txt"});
    stopped(
        &site
            .done_with("work", "trash", args, Seen::new(), raised())
            .await,
    );
    assert_eq!(content(&site, "work/old.txt"), b"x");
}

#[tokio::test]
async fn the_checks_before_the_flag_still_answer_first() {
    // 旗只管「改不改」：参数不对、没看过的，照旧出错，和没举旗一样。
    let site = Site::new();
    site.file("work/c.txt", b"c");
    let args = json!({"file_path": "c.txt", "content": "new"});
    let done = site
        .done_with("work", "write", args, Seen::new(), raised())
        .await;
    assert!(done.error && !done.stopped, "{done:?}");
    assert_eq!(content(&site, "work/c.txt"), b"c");
}
