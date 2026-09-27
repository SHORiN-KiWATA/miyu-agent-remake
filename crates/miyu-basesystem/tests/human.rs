//! 三件读的工具、写的三件（施工 4-6）、`shell`（施工 4-8）交回的给人看的说法（施工 4-5 上）：每一种结果都有，编号、字段对；工具会说的每一种，中文、英文
//! 两份字里都有，换得出字。

mod support;

use miyu_kernel::event::Said;
use miyu_kernel::id::ContentHash;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_tool::{Done, Seen};

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

    readable(&checked, &["read", "glob", "grep"]);
}

/// 会说的每一种，中文、英文两份字里都有，换得出字；这几件工具都有显示名。
fn readable(checked: &[Said], tools: &[&str]) {
    let root = ResourceRoot::at(resources());
    for language in ["zh", "en"] {
        let words = Human::load(&root, language).expect("给人看的字读得出来");
        for said in checked {
            assert!(words.say(said).is_some(), "{language} 没有 {said:?}");
        }
        for tool in tools {
            assert!(
                words.tool(tool).is_some(),
                "{language} 没有 {tool} 的显示名"
            );
        }
    }
}

#[tokio::test]
async fn every_write_outcome_says_something_people_can_read() {
    let site = Site::new();
    site.file("work/old.txt", b"old\n");
    let real = site.real("work/old.txt");
    let run = |args: serde_json::Value, seen: Seen| site.done_seen("work", "write", args, seen);
    let mut checked = Vec::new();
    let saw = |content: &[u8]| Seen::from([(real.clone(), ContentHash::of(content))]);
    check(
        &mut checked,
        human(
            run(
                serde_json::json!({"file_path": "new.txt", "content": "a\nb\n"}),
                Seen::new(),
            )
            .await,
        ),
        said("write/created").with("count", "2"),
    );
    check(
        &mut checked,
        human(
            run(
                serde_json::json!({"file_path": "old.txt", "content": "x"}),
                Seen::new(),
            )
            .await,
        ),
        said("common/not-read"),
    );
    check(
        &mut checked,
        human(
            run(
                serde_json::json!({"file_path": "old.txt", "content": "x"}),
                saw(b"other"),
            )
            .await,
        ),
        said("common/stale"),
    );
    check(
        &mut checked,
        human(
            run(
                serde_json::json!({"file_path": "old.txt", "content": "x"}),
                saw(b"old\n"),
            )
            .await,
        ),
        said("write/updated").with("count", "1"),
    );
    check(
        &mut checked,
        human(
            run(
                serde_json::json!({"file_path": ".", "content": "x"}),
                Seen::new(),
            )
            .await,
        ),
        said("common/directory"),
    );
    // 往一个文件底下写：哪个平台都写不了，原话各平台不一样，只核对是哪一句。
    let failed = human(
        run(
            serde_json::json!({"file_path": "old.txt/x", "content": "x"}),
            Seen::new(),
        )
        .await,
    );
    assert_eq!(failed.key, said("common/write-failed").key);
    assert!(failed.fields.contains_key("error"), "{failed:?}");
    checked.push(failed);
    checked.push(said("common/not-a-regular-file"));
    readable(&checked, &["write"]);
}

#[tokio::test]
async fn every_edit_outcome_says_something_people_can_read() {
    let site = Site::new();
    site.file("work/a.txt", b"alpha\nbeta\nbeta\n");
    let seen = Seen::from([(
        site.real("work/a.txt"),
        ContentHash::of(b"alpha\nbeta\nbeta\n"),
    )]);
    let run = |args: serde_json::Value| site.done_seen("work", "edit", args, seen.clone());
    let edit = |old: &str, new: &str| serde_json::json!({"file_path": "a.txt", "old_string": old, "new_string": new});
    let mut checked = Vec::new();
    check(
        &mut checked,
        human(run(serde_json::json!({"file_path": "a.txt"})).await),
        said("edit/no-edits"),
    );
    check(
        &mut checked,
        human(run(edit("", "x")).await),
        said("edit/empty").with("index", "1"),
    );
    check(
        &mut checked,
        human(run(edit("beta", "beta")).await),
        said("edit/same").with("index", "1"),
    );
    check(
        &mut checked,
        human(run(edit("zzzz qqqq", "x")).await),
        said("edit/not-found").with("index", "1"),
    );
    check(
        &mut checked,
        human(run(edit("alphx", "x")).await),
        said("edit/not-found-near")
            .with("index", "1")
            .with("line", "1"),
    );
    check(
        &mut checked,
        human(run(edit("beta", "x")).await),
        said("edit/not-unique")
            .with("index", "1")
            .with("count", "2"),
    );
    check(
        &mut checked,
        human(
            run(serde_json::json!({"file_path": "a.txt", "edits": [
                {"old_string": "alpha\nbeta", "new_string": "x"},
                {"old_string": "alpha", "new_string": "y"},
            ]}))
            .await,
        ),
        said("edit/overlap").with("first", "1").with("second", "2"),
    );
    site.file("work/bin", b"\xFF\x00");
    let bin = Seen::from([(site.real("work/bin"), ContentHash::of(b"\xFF\x00"))]);
    check(
        &mut checked,
        human(
            site.done_seen(
                "work",
                "edit",
                serde_json::json!({"file_path": "bin", "old_string": "a", "new_string": "b"}),
                bin,
            )
            .await,
        ),
        said("edit/not-text"),
    );
    check(
        &mut checked,
        human(run(edit("alpha", "a")).await),
        said("edit/edited").with("count", "1"),
    );
    readable(&checked, &["edit"]);
}

#[tokio::test]
async fn every_trash_outcome_says_something_people_can_read() {
    let site = Site::new();
    site.file("work/a.txt", b"a\n");
    let run = |path: &str| site.done("trash", serde_json::json!({ "file_path": path }));
    let mut checked = Vec::new();
    check(&mut checked, human(run(".").await), said("trash/protected"));
    // 删成了的，这台机器上真删进回收站：Linux 上是场地里假家目录的回收站。
    #[cfg(target_os = "linux")]
    check(
        &mut checked,
        human(run("a.txt").await),
        said("trash/trashed"),
    );
    #[cfg(not(target_os = "linux"))]
    checked.push(said("trash/trashed"));
    checked.push(said("trash/unavailable"));
    checked.push(said("trash/lost"));
    checked.push(said("trash/failed").with("error", "Permission denied"));
    readable(&checked, &["trash"]);
}

#[tokio::test]
async fn every_shell_outcome_says_something_people_can_read() {
    let site = Site::new();
    let windows = cfg!(windows);
    let run = |args: serde_json::Value| site.done("shell", args);
    let command = |command: &str| serde_json::json!({ "command": command });
    let mut checked = Vec::new();
    check(
        &mut checked,
        human(run(command(if windows { "Write-Output a" } else { "echo a" })).await),
        said("shell/done").with("count", "1"),
    );
    check(
        &mut checked,
        human(run(command(if windows { "$null = 1" } else { "true" })).await),
        said("shell/quiet"),
    );
    check(
        &mut checked,
        human(run(command("exit 4")).await),
        said("shell/exited").with("code", "4"),
    );
    let sleep = if windows {
        "Start-Sleep -Seconds 20"
    } else {
        "sleep 20"
    };
    check(
        &mut checked,
        human(run(serde_json::json!({ "command": sleep, "timeout": 200 })).await),
        said("shell/timed-out").with("seconds", "0.2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({ "command": "exit 0", "run_in_background": true })).await),
        said("shell/no-background"),
    );
    // 起不来的：工作目录不在。
    let failed = human(site.done_in("nowhere", "shell", command("exit 0")).await);
    assert!(failed.key.ends_with("shell/failed"), "{failed:?}");
    checked.push(failed);
    #[cfg(unix)]
    check(
        &mut checked,
        human(run(command("kill -9 $$")).await),
        said("shell/signal").with("signal", "9"),
    );
    #[cfg(not(unix))]
    checked.push(said("shell/signal").with("signal", "9"));
    readable(&checked, &["shell"]);
}
