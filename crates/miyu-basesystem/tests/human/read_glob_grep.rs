//! `read`、`glob`、`grep` 交回的给人看的说法（施工 4-5 上；4-5 再补加了「数是 1 接 `/one`」的几句）：拆自
//! `human.rs`（照 `send_message/by_session_id.rs` 的先例，那个文件超过了行数上限）。

use miyu_kernel::event::Said;

use super::{Site, check, human, readable, said};

#[tokio::test]
async fn every_outcome_says_something_people_can_read() {
    let site = Site::new();
    site.file("work/a.txt", b"one\ntwo\n");
    site.file("work/empty.txt", b"");
    site.file("work/app.bin", b"\x7fELF\0\0");
    site.file("work/notes.txt", b"n");
    site.file("work/dir/x.rs", b"fn x() {}\n");
    site.file("work/dir/y.rs", b"fn y() {}\n");
    for n in 0..101 {
        site.file(&format!("work/many/{n:03}.md"), b"");
    }
    site.aged("work/dir/y.rs", 100);
    // 只有一项、一处的几种：测「数是 1」接 `/one`（施工 4-5 再补）。`dir/` 底下不能加（`read/entries` 数着
    // 它正好 2 项），另起一个 `extra/`。
    site.file("work/one/solo.txt", b"");
    site.file("work/extra/alone.ini", b"");
    site.file("work/extra/zzzmatch.txt", b"zzzmatch\n");
    let mut checked: Vec<Said> = Vec::new();

    // read
    let run = |args: serde_json::Value| site.done("read", args);
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "a.txt"})).await),
        said("read/lines").with("count", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "notes.txt"})).await),
        said("read/lines/one").with("count", "1"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "a.txt", "offset": 2})).await),
        said("read/lines-part")
            .with("from", "2")
            .with("to", "2")
            .with("total", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "dir"})).await),
        said("read/entries").with("count", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "one"})).await),
        said("read/entries/one").with("count", "1"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "dir", "limit": 1})).await),
        said("read/entries-part")
            .with("from", "1")
            .with("to", "1")
            .with("total", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "empty.txt"})).await),
        said("read/empty"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "a.txt", "offset": 9})).await),
        said("read/past-end").with("total", "2").with("offset", "9"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "notes.txt", "offset": 9})).await),
        said("read/past-end/one")
            .with("total", "1")
            .with("offset", "9"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "dir", "offset": 9})).await),
        said("read/past-end-entries")
            .with("total", "2")
            .with("offset", "9"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "one", "offset": 9})).await),
        said("read/past-end-entries/one")
            .with("total", "1")
            .with("offset", "9"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "app.bin"})).await),
        said("read/binary").with("path", "app.bin"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "nope.zzz"})).await),
        said("common/missing").with("path", "nope.zzz"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "note.txt"})).await),
        said("common/missing-similar")
            .with("path", "note.txt")
            .with("similar", "notes.txt"),
    );
    let bad = human(run(serde_json::json!({"offset": 1})).await);
    assert_eq!(bad.key, "software/basesystem/common/bad-args");
    assert!(bad.fields["error"].contains("file_path"), "{bad:?}");
    checked.push(bad);
    #[cfg(unix)]
    {
        let fifo = site.0.join("work/pipe");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .expect("有 mkfifo")
                .success()
        );
        check(
            &mut checked,
            human(run(serde_json::json!({"file_path": "pipe"})).await),
            said("read/not-a-file").with("path", "pipe"),
        );
    }

    // glob
    let run = |args: serde_json::Value| site.done("glob", args);
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "*.rs"})).await),
        said("glob/files").with("count", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "*.ini"})).await),
        said("glob/files/one").with("count", "1"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "*.md"})).await),
        said("glob/files-more")
            .with("shown", "100")
            .with("total", "101"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "*.py"})).await),
        said("common/no-files"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "*.rs", "path": "a.txt"})).await),
        said("glob/not-a-directory").with("path", "a.txt"),
    );
    let bad = human(run(serde_json::json!({"pattern": "[ab"})).await);
    assert_eq!(bad.key, "software/basesystem/common/bad-glob");
    assert_eq!(bad.fields["glob"], "[ab");
    checked.push(bad);

    // grep
    let run = |args: serde_json::Value| site.done("grep", args);
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "fn"})).await),
        said("grep/files").with("count", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "zzzmatch"})).await),
        said("grep/files/one").with("count", "1"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "fn", "head_limit": 1})).await),
        said("grep/files-part")
            .with("from", "1")
            .with("to", "1")
            .with("total", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "fn", "output_mode": "count"})).await),
        said("grep/counts").with("count", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "zzzmatch", "output_mode": "count"})).await),
        said("grep/counts/one").with("count", "1"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "fn", "output_mode": "count", "offset": 1})).await),
        said("grep/counts-part")
            .with("from", "2")
            .with("to", "2")
            .with("total", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "fn", "output_mode": "content"})).await),
        said("grep/matches").with("count", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "zzzmatch", "output_mode": "content"})).await),
        said("grep/matches/one").with("count", "1"),
    );
    check(
        &mut checked,
        human(
            run(serde_json::json!({"pattern": "fn", "output_mode": "content", "offset": 1})).await,
        ),
        said("grep/matches-part").with("from", "2").with("to", "2"),
    );
    check(
        &mut checked,
        human(
            run(serde_json::json!({"pattern": "fn", "output_mode": "content", "head_limit": 1}))
                .await,
        ),
        said("grep/matches-more").with("from", "1").with("to", "1"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "zebra"})).await),
        said("grep/none"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "zebra", "output_mode": "content"})).await),
        said("grep/none"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "fn", "offset": 9})).await),
        said("grep/past-end").with("total", "2").with("offset", "9"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "zzzmatch", "offset": 9})).await),
        said("grep/past-end/one")
            .with("total", "1")
            .with("offset", "9"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"pattern": "(fn"})).await),
        said("grep/bad-pattern").with("error", "unclosed group"),
    );

    readable(&checked, &["read", "glob", "grep"]);
}
