//! `write`（施工 4-6 上）：新建（带建上级目录）、覆盖；没读过的、读过以后被改了的不写，自己刚写过的可以接着写；
//! 目录、只读的不写；覆盖时照原来的 CRLF、BOM、UTF-16；报的 `file.changed` 改前改后对得上；给人看的说法。

mod support;

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_kernel::event::Said;
use miyu_kernel::id::ContentHash;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect, Seen};

use support::{Site, native, tool};

/// 基础系统的说法。
fn said(key: &str) -> Said {
    Said::new(format!("software/basesystem/{key}"))
}

/// 交回的字。
fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 结果里的路径：相对工作目录、照本平台的分隔符，照模板转义过、带引号。
fn quoted(path: &str) -> String {
    serde_json::to_string(&native(path)).expect("字写得成 JSON")
}

/// 她看过场地里的这几个文件，看到的就是它们现在的内容。
fn seen_now(site: &Site, paths: &[&str]) -> Seen {
    paths
        .iter()
        .map(|path| {
            let bytes = std::fs::read(site.0.join(path)).expect("读得出");
            (site.real(path), ContentHash::of(&bytes))
        })
        .collect()
}

/// 在 `work/` 里写 `file_path`，内容是 `content`，她看过的是 `seen`。
async fn write(site: &Site, file_path: &str, content: &str, seen: Seen) -> Done {
    site.done_seen(
        "work",
        "write",
        json!({"file_path": file_path, "content": content}),
        seen,
    )
    .await
}

#[test]
fn write_comes_from_the_resources_and_names_what_it_writes() {
    let tool = tool("write");
    assert_eq!(tool.spec().access, Access::Write);
    let targets = tool.targets(&Call {
        args: json!({"file_path": "src/a.rs", "content": ""}).to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Default::default(),
        stop: Default::default(),
        sandbox: None,
        log: None,
        offset: miyu_kernel::time::UtcOffset::UTC,
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
    });
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].path, "src/a.rs");
    assert!(targets[0].write, "是要写的");
}

#[tokio::test]
async fn a_new_file_is_created_with_its_folders() {
    let site = Site::new();
    let done = write(&site, "src/new/a.rs", "fn a() {}\n", Seen::new()).await;
    assert!(!done.error, "{done:?}");
    assert_eq!(
        text(&done),
        format!("Created {}.\n", quoted("src/new/a.rs"))
    );
    assert_eq!(
        std::fs::read(site.0.join("work/src/new/a.rs")).unwrap(),
        b"fn a() {}\n"
    );
    assert_eq!(
        done.human,
        Some(said("write/created/one").with("count", "1"))
    );
    assert_eq!(
        done.effects,
        [Effect::Changed {
            path: site.real("work/src/new/a.rs"),
            before: None,
            after: b"fn a() {}\n".to_vec(),
        }]
    );
    // 给的是工作区里的绝对路径：结果里照样写相对的。
    let absolute = site.real("work").join("b.txt");
    let done = write(&site, &absolute.to_string_lossy(), "b\n", Seen::new()).await;
    assert_eq!(text(&done), format!("Created {}.\n", quoted("b.txt")));
}

/// 数是 0 的时候不是 1，照旧不接 `/one`（施工 4-5 再补「一个的时候说单数」）。
#[tokio::test]
async fn an_empty_new_file_keeps_the_plain_key() {
    let site = Site::new();
    let done = write(&site, "empty.txt", "", Seen::new()).await;
    assert!(!done.error, "{done:?}");
    assert_eq!(done.human, Some(said("write/created").with("count", "0")));
}

#[tokio::test]
async fn an_existing_file_needs_to_have_been_seen_as_it_is() {
    let site = Site::new();
    site.file("work/a.txt", b"old\n");
    let done = write(&site, "a.txt", "new\n", Seen::new()).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        format!(
            "{} already exists and has not been read. Read it first.\n",
            quoted("a.txt")
        )
    );
    assert_eq!(done.human, Some(said("common/not-read")));
    let seen = seen_now(&site, &["work/a.txt"]);
    // 她看过以后，文件被别人改了。
    site.file("work/a.txt", b"changed by someone\n");
    let done = write(&site, "a.txt", "new\n", seen).await;
    assert!(done.error);
    assert_eq!(done.human, Some(said("common/stale")));
    assert_eq!(
        std::fs::read(site.0.join("work/a.txt")).unwrap(),
        b"changed by someone\n",
        "没写"
    );
    // 看的是现在的样子：写。
    let done = write(
        &site,
        "a.txt",
        "new\nlines\n",
        seen_now(&site, &["work/a.txt"]),
    )
    .await;
    assert!(!done.error, "{done:?}");
    assert_eq!(text(&done), format!("Updated {}.\n", quoted("a.txt")));
    assert_eq!(done.human, Some(said("write/updated").with("count", "2")));
    assert_eq!(
        done.effects,
        [Effect::Changed {
            path: site.real("work/a.txt"),
            before: Some(b"changed by someone\n".to_vec()),
            after: b"new\nlines\n".to_vec(),
        }]
    );
    // 自己刚写过的：她看到的就是改后的，可以接着写。
    let seen = Seen::from([(site.real("work/a.txt"), ContentHash::of(b"new\nlines\n"))]);
    let done = write(&site, "a.txt", "again\n", seen).await;
    assert!(!done.error, "{done:?}");
    assert_eq!(
        std::fs::read(site.0.join("work/a.txt")).unwrap(),
        b"again\n"
    );
}

#[tokio::test]
async fn an_overwrite_keeps_the_old_line_endings_bom_and_encoding() {
    let site = Site::new();
    let cases: [(&str, &[u8], &[u8]); 4] = [
        ("crlf.txt", b"a\r\nb\r\n", b"x\r\ny\r\n"),
        ("bom.txt", b"\xEF\xBB\xBFa\n", b"\xEF\xBB\xBFx\ny\n"),
        (
            "utf16.txt",
            b"\xFF\xFEa\x00\n\x00",
            b"\xFF\xFEx\x00\n\x00y\x00\n\x00",
        ),
        ("lf.txt", b"a\nb\n", b"x\ny\n"),
    ];
    for (name, old, want) in cases {
        let path = format!("work/{name}");
        site.file(&path, old);
        let done = write(&site, name, "x\ny\n", seen_now(&site, &[&path])).await;
        assert!(!done.error, "{name}: {done:?}");
        assert_eq!(std::fs::read(site.0.join(&path)).unwrap(), want, "{name}");
    }
}

#[tokio::test]
async fn directories_and_read_only_files_are_not_written() {
    let site = Site::new();
    std::fs::create_dir_all(site.0.join("work/dir")).unwrap();
    let done = write(&site, "dir", "x", seen_now(&site, &[])).await;
    assert!(done.error);
    assert_eq!(text(&done), format!("{} is a directory.\n", quoted("dir")));
    assert_eq!(done.human, Some(said("common/directory")));
    site.file("work/locked.txt", b"keep\n");
    let file = site.0.join("work/locked.txt");
    let mut permissions = std::fs::metadata(&file).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&file, permissions).unwrap();
    let done = write(
        &site,
        "locked.txt",
        "x",
        seen_now(&site, &["work/locked.txt"]),
    )
    .await;
    assert!(done.error, "{done:?}");
    assert!(
        text(&done).starts_with(&format!("Could not write {}: ", quoted("locked.txt"))),
        "{}",
        text(&done)
    );
    assert!(
        matches!(&done.human, Some(said) if said.key == "software/basesystem/common/write-failed")
    );
    assert_eq!(std::fs::read(&file).unwrap(), b"keep\n");
    // 删场地之前放开只读，Windows 上删不掉只读的文件。
    let mut permissions = std::fs::metadata(&file).unwrap().permissions();
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "测试收尾：放开只读好删掉场地"
    )]
    permissions.set_readonly(false);
    std::fs::set_permissions(&file, permissions).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn the_old_permissions_stay_and_no_temporary_file_is_left() {
    use std::os::unix::fs::PermissionsExt;
    let site = Site::new();
    site.file("work/run.sh", b"echo old\n");
    let file = site.0.join("work/run.sh");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o750)).unwrap();
    let done = write(
        &site,
        "run.sh",
        "echo new\n",
        seen_now(&site, &["work/run.sh"]),
    )
    .await;
    assert!(!done.error, "{done:?}");
    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o750);
    let names: Vec<String> = std::fs::read_dir(site.0.join("work"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["run.sh"], "临时文件改名盖上去了，没留下别的");
}

#[cfg(unix)]
#[tokio::test]
async fn something_that_is_not_a_regular_file_is_not_written() {
    let site = Site::new();
    let done = site
        .done_seen(
            "work",
            "write",
            json!({"file_path": "/dev/null", "content": "x"}),
            Seen::new(),
        )
        .await;
    assert!(done.error);
    assert_eq!(done.human, Some(said("common/not-a-regular-file")));
}

/// 读过的二进制文件盖得了（施工 4-9 再补二）：`read` 读到二进制不给内容，照样报读过，`write` 照它核对。
#[tokio::test]
async fn a_binary_file_she_read_can_be_overwritten() {
    let site = Site::new();
    site.file("work/logo.bin", b"\x89PNG\0\0data");
    let read = site.done("read", json!({"file_path": "logo.bin"})).await;
    let seen: Seen = read
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Read { path, hash, .. } => Some((path.clone(), hash.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(seen.len(), 1, "读二进制也报读过");
    let done = write(&site, "logo.bin", "text now", seen).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        std::fs::read(site.0.join("work/logo.bin")).expect("在"),
        b"text now"
    );
}

/// 碰到 FIFO：`write`、`edit` 照「安全地打开」开，说不是普通文件，不卡住（施工 4-9 再补二）。
#[cfg(unix)]
#[tokio::test]
async fn a_fifo_is_not_a_regular_file() {
    let site = Site::new();
    let fifo = site.real("work").join("pipe");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("有 mkfifo");
    assert!(made.success());
    for (name, args) in [
        ("write", json!({"file_path": "pipe", "content": "x"})),
        (
            "edit",
            json!({"file_path": "pipe", "edits": [{"old_string": "a", "new_string": "b"}]}),
        ),
    ] {
        let done = site.done_seen("work", name, args, Seen::new()).await;
        assert_eq!(
            done.human,
            Some(said("common/not-a-regular-file")),
            "{name}：{}",
            text(&done)
        );
    }
}
