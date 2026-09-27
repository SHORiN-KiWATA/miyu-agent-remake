//! 三件读的工具交回的给人看的说法（施工 4-5 上）：每一种结果都有，编号、字段对；工具会说的每一种，中文、英文
//! 两份字里都有，换得出字。

mod support;

use miyu_kernel::event::Said;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Done;

use support::{Site, resources};

/// 基础系统的说法。
fn said(key: &str) -> Said {
    Said::new(format!("software/basesystem/{key}"))
}

/// 核对 `got` 就是 `want`，记下来，最后一起查两份字里有没有。
fn check(checked: &mut Vec<Said>, got: Said, want: Said) {
    assert_eq!(got, want);
    checked.push(got);
}

/// 这次调用给人看的说法。
fn human(done: Done) -> Said {
    done.human.expect("每一种结果都带说法")
}

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
        human(run(serde_json::json!({"file_path": "dir", "offset": 9})).await),
        said("read/past-end-entries")
            .with("total", "2")
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
        human(run(serde_json::json!({"pattern": "(fn"})).await),
        said("grep/bad-pattern").with("error", "unclosed group"),
    );

    // 会说的每一种，中文、英文两份字里都有，换得出字。
    let root = ResourceRoot::at(resources());
    for language in ["zh", "en"] {
        let words = Human::load(&root, language).expect("给人看的字读得出来");
        for said in &checked {
            assert!(words.say(said).is_some(), "{language} 没有 {said:?}");
        }
        for tool in ["read", "glob", "grep"] {
            assert!(
                words.tool(tool).is_some(),
                "{language} 没有 {tool} 的显示名"
            );
        }
    }
}
