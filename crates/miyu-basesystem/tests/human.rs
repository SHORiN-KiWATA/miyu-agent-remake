//! 写的三件（施工 4-6）、`shell`（施工 4-8）交回的给人看的说法（施工 4-5 上）：每一种结果都有，编号、字段对；工具会说的每一种，中文、英文
//! 两份字里都有，换得出字。数管着的是 1 的编号多接 `/one`，别的数照旧（施工 4-5 再补「一个的时候说单数」）；
//! `read`、`glob`、`grep` 那一组拆进 `human/read_glob_grep.rs`，理由同下面那个 `mod`。

use miyu_kernel::id::ContentHash;
use miyu_tool::Seen;

use crate::support::{Site, check, human, readable, said};

/// `read`、`glob`、`grep` 交回的说法：这个文件超过了行数上限，拆进这里（照 `spawn/renamed.rs` 的先例，
/// `#[path]` 一样要写：这个文件是 crate 根，`mod` 默认只找同目录的平级文件）。
#[path = "human/read_glob_grep.rs"]
mod read_glob_grep;

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
        said("write/updated/one").with("count", "1"),
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
    // 改了不止一处：照旧，编号不接 `/one`。独立的一份文件，不碰上面那份 `a.txt`：它的 `seen` 是造场地时记的
    // 原文，这里真的改了盘上的文件，用同一份会撞「她看过的」不是现在这份。
    site.file("work/counted.txt", b"beta\nbeta\n");
    let counted = Seen::from([(
        site.real("work/counted.txt"),
        ContentHash::of(b"beta\nbeta\n"),
    )]);
    check(
        &mut checked,
        human(
            site.done_seen(
                "work",
                "edit",
                serde_json::json!({
                    "file_path": "counted.txt",
                    "old_string": "beta",
                    "new_string": "gamma",
                    "replace_all": true,
                }),
                counted,
            )
            .await,
        ),
        said("edit/edited").with("count", "2"),
    );
    check(
        &mut checked,
        human(run(edit("alpha", "a")).await),
        said("edit/edited/one").with("count", "1"),
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
    let command = |command: &str| serde_json::json!({ "command": command, "description": "Test" });
    let mut checked = Vec::new();
    check(
        &mut checked,
        human(run(command(if windows { "Write-Output a" } else { "echo a" })).await),
        said("shell/done/one").with("count", "1"),
    );
    check(
        &mut checked,
        human(
            run(command(if windows {
                "Write-Output a; Write-Output b"
            } else {
                "printf 'a\\nb\\n'"
            }))
            .await,
        ),
        said("shell/done").with("count", "2"),
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
        human(
            run(serde_json::json!({ "command": sleep, "description": "Test", "timeout": 200 }))
                .await,
        ),
        said("shell/timed-out").with("seconds", "0.2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({ "command": "exit 0", "description": "Test", "run_in_background": true })).await),
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
