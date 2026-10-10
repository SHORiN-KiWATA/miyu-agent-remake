//! `read`（施工 4-4 上，施工 4-4 下改到规范上）：从资源目录造出来；读文件带行号、翻页；读目录、一样翻页；
//! 读不了的说清楚，找不到的列出相近的名字。

use miyu_kernel::id::ContentHash;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Effect};

use crate::support::{Site, resources, tool};

#[test]
fn read_comes_from_the_resources_with_its_schema_as_written() {
    let tool = tool("read");
    let spec = tool.spec();
    assert_eq!(spec.access, Access::Read);
    assert!(
        spec.description.starts_with(
            "Read a text file or an image (PNG, JPEG, GIF, WebP), or list a directory."
        ),
        "{}",
        spec.description
    );
    assert!(spec.description.contains("cat -n format"));
    assert_eq!(
        spec.parameters.get(),
        r#"{"type":"object","properties":{"file_path":{"type":"string","description":"Absolute, or relative to the working directory."},"offset":{"type":"integer","description":"The line number to start reading from, counting from 1."},"limit":{"type":"integer","description":"The number of lines to read. Default 2000."}},"required":["file_path"]}"#
    );
    let targets = tool.targets(&Call {
        args: r#"{"file_path":"src/a.rs"}"#.to_string(),
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
        usage: None,
        questions: None,
        memory: None,
        packages: None,
        ids: None,
    });
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].path, "src/a.rs");
    assert!(!targets[0].write);
}

#[test]
fn a_broken_resource_is_named() {
    let site = Site::new();
    // 只有说明、没有输出里的几句的资源目录：说是缺的那一份，先读的是几件共用的。
    let broken = site.0.join("resources");
    let tools = broken.join("software").join("basesystem").join("tools");
    std::fs::create_dir_all(&tools).expect("建得了目录");
    for name in ["read", "glob", "grep"] {
        std::fs::copy(
            resources().join(format!("software/basesystem/tools/{name}.json")),
            tools.join(format!("{name}.json")),
        )
        .expect("拷得了");
    }
    let Err(error) = miyu_basesystem::tools(&broken) else {
        panic!("读不全");
    };
    assert!(
        error.file.ends_with("common/missing.txt"),
        "{}",
        error.file.display()
    );
    // 共用的几句都在，read 的说明写坏了：说是哪一份。
    let common = broken.join("software").join("basesystem").join("common");
    std::fs::create_dir_all(&common).expect("建得了目录");
    for entry in std::fs::read_dir(resources().join("software/basesystem/common")).expect("在") {
        let entry = entry.expect("读得了");
        std::fs::copy(entry.path(), common.join(entry.file_name())).expect("拷得了");
    }
    std::fs::write(tools.join("read.json"), "{").expect("写得进");
    let Err(error) = miyu_basesystem::tools(&broken) else {
        panic!("读不懂");
    };
    assert!(error.file.ends_with("read.json"), "{error}");
}

#[tokio::test]
async fn a_file_reads_with_line_numbers_and_pages_on() {
    let site = Site::new();
    let body: String = (1..=5).map(|n| format!("line {n}\n")).collect();
    site.file("work/notes.txt", body.as_bytes());
    let (error, text) = site
        .call("read", serde_json::json!({"file_path": "notes.txt"}))
        .await;
    assert!(!error);
    // 行号、一个制表符、原文：行号前不补空格。
    assert_eq!(
        text,
        "1\tline 1\n2\tline 2\n3\tline 3\n4\tline 4\n5\tline 5\n"
    );
    let (_, text) = site
        .call(
            "read",
            serde_json::json!({"file_path": "notes.txt", "offset": 2, "limit": 2}),
        )
        .await;
    assert_eq!(
        text,
        "2\tline 2\n3\tline 3\n(Showing lines 2-3 of 5. Use offset=4 to continue.)\n"
    );
    let (error, text) = site
        .call(
            "read",
            serde_json::json!({"file_path": "notes.txt", "offset": 9}),
        )
        .await;
    assert!(!error);
    assert_eq!(text, "(The file has 5 lines; offset 9 is past the end.)\n");
}

#[tokio::test]
async fn path_and_file_path_spelled_the_other_ways_are_read_too() {
    let site = Site::new();
    site.file("work/a.txt", b"a\n");
    for args in [
        serde_json::json!({"path": "a.txt"}),
        serde_json::json!({"filePath": "a.txt"}),
    ] {
        assert_eq!(site.call("read", args).await, (false, "1\ta\n".to_string()));
    }
}

#[tokio::test]
async fn home_and_empty_and_binary() {
    let site = Site::new();
    site.file("home/plan.md", b"# plan\n");
    let (error, text) = site
        .call("read", serde_json::json!({"file_path": "~/plan.md"}))
        .await;
    assert!(!error);
    assert_eq!(text, "1\t# plan\n");
    site.file("work/empty.txt", b"");
    assert_eq!(
        site.call("read", serde_json::json!({"file_path": "empty.txt"}))
            .await,
        (false, "(It is empty.)\n".to_string())
    );
    site.file("work/app.bin", b"\x7fELF\0\0");
    assert_eq!(
        site.call("read", serde_json::json!({"file_path": "app.bin"}))
            .await,
        (true, "\"app.bin\" is a binary file.\n".to_string())
    );
}

#[tokio::test]
async fn a_directory_lists_its_entries_by_name_and_pages_on() {
    let site = Site::new();
    site.file("work/b.txt", b"b");
    site.file("work/a/inner.txt", b"a");
    site.file("work/c.rs", b"c");
    let (error, text) = site
        .call("read", serde_json::json!({"file_path": "."}))
        .await;
    assert!(!error);
    assert_eq!(text, "a/\nb.txt\nc.rs\n");
    std::fs::create_dir_all(site.0.join("work/hollow")).expect("建得了");
    assert_eq!(
        site.call("read", serde_json::json!({"file_path": "hollow"}))
            .await,
        (false, "(It is empty.)\n".to_string())
    );
    for n in 0..1003 {
        site.file(&format!("work/many/{n:04}.txt"), b"x");
    }
    // 不给 limit，一次最多 2000 项：1003 项全列。
    let (_, text) = site
        .call("read", serde_json::json!({"file_path": "many"}))
        .await;
    assert_eq!(text.lines().count(), 1003);
    // 照 offset、limit 翻页，和文件一样。
    let (_, text) = site
        .call(
            "read",
            serde_json::json!({"file_path": "many", "limit": 1000}),
        )
        .await;
    assert_eq!(text.lines().count(), 1001);
    assert!(
        text.ends_with("(Showing entries 1-1000 of 1003. Use offset=1001 to continue.)\n"),
        "{}",
        &text[text.len() - 80..]
    );
    let (_, text) = site
        .call(
            "read",
            serde_json::json!({"file_path": "many", "offset": 1001}),
        )
        .await;
    assert_eq!(text, "1000.txt\n1001.txt\n1002.txt\n");
    let (error, text) = site
        .call(
            "read",
            serde_json::json!({"file_path": "many", "offset": 2000}),
        )
        .await;
    assert!(!error);
    assert_eq!(
        text,
        "(The directory has 1003 entries; offset 2000 is past the end.)\n"
    );
}

#[tokio::test]
async fn what_cannot_be_read_says_so() {
    let site = Site::new();
    assert_eq!(
        site.call("read", serde_json::json!({"file_path": "nope.txt"}))
            .await,
        (
            true,
            "There is no file or directory at \"nope.txt\".\n".to_string()
        )
    );
    let (error, text) = site.call("read", serde_json::json!({"offset": 1})).await;
    assert!(error);
    assert!(text.starts_with("The arguments are not right: "), "{text}");
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
        assert_eq!(
            site.call("read", serde_json::json!({"file_path": "pipe"}))
                .await,
            (
                true,
                "\"pipe\" is not a regular file or a directory.\n".to_string()
            )
        );
    }
}

#[tokio::test]
async fn a_missing_file_lists_similar_names_next_to_it() {
    let site = Site::new();
    site.file("work/notes.txt", b"n");
    site.file("work/src/main.rs", b"fn main() {}");
    site.file("work/src/lib.rs", b"");
    assert_eq!(
        site.call("read", serde_json::json!({"file_path": "note.txt"}))
            .await,
        (
            true,
            "There is no file or directory at \"note.txt\".\nDid you mean \"notes.txt\"?\n"
                .to_string()
        )
    );
    // 在下一层找不到的：照同一个目录找相近的，路径照工作目录写。
    let (_, text) = site
        .call("read", serde_json::json!({"file_path": "src/main.ts"}))
        .await;
    assert_eq!(
        text,
        format!(
            "There is no file or directory at \"src/main.ts\".\nDid you mean \"{}\"?\n",
            crate::support::native("src/main.rs").replace('\\', "\\\\")
        )
    );
    // 一个相近的都没有：只说没有。
    let (_, text) = site
        .call("read", serde_json::json!({"file_path": "src/zzz.py"}))
        .await;
    assert_eq!(text, "There is no file or directory at \"src/zzz.py\".\n");
}

#[tokio::test]
async fn one_read_is_at_most_two_thousand_lines() {
    let site = Site::new();
    let body: String = (1..=2500).map(|n| format!("{n}\n")).collect();
    site.file("work/long.txt", body.as_bytes());
    for args in [
        serde_json::json!({"file_path": "long.txt"}),
        serde_json::json!({"file_path": "long.txt", "limit": 5000}),
    ] {
        let (_, text) = site.call("read", args).await;
        assert_eq!(text.lines().count(), 2001, "2000 行加一句还没读完");
        assert!(
            text.ends_with("(Showing lines 1-2000 of 2500. Use offset=2001 to continue.)\n"),
            "{}",
            &text[text.len() - 80..]
        );
    }
}

#[tokio::test]
async fn reading_a_file_reports_what_was_read_with_the_whole_hash() {
    let site = Site::new();
    let whole = b"one\ntwo\nthree\n";
    site.file("work/a.txt", whole);
    site.file("work/empty.txt", b"");
    site.file("work/bin", b"a\x00b");
    std::fs::create_dir_all(site.0.join("work/dir")).unwrap();
    let real = site.real("work/a.txt");
    let read = |lines: Option<[u64; 2]>| Effect::Read {
        path: real.clone(),
        lines,
        hash: ContentHash::of(whole),
    };
    let effects = |args: serde_json::Value| async { site.done("read", args).await.effects };
    assert_eq!(
        effects(serde_json::json!({"file_path": "a.txt"})).await,
        [read(Some([1, 3]))]
    );
    assert_eq!(
        effects(serde_json::json!({"file_path": "a.txt", "offset": 2, "limit": 1})).await,
        [read(Some([2, 2]))],
        "读一段的，哈希也是整份的"
    );
    assert_eq!(
        effects(serde_json::json!({"file_path": "a.txt", "offset": 9})).await,
        [read(None)],
        "过了结尾的一行都没显示"
    );
    assert_eq!(
        effects(serde_json::json!({"file_path": "empty.txt"})).await,
        [Effect::Read {
            path: site.real("work/empty.txt"),
            lines: None,
            hash: ContentHash::of(b""),
        }]
    );
    // 二进制的不给内容，照样报读过（施工 4-9 再补二）：`write` 盖它之前照它核对。
    assert_eq!(
        effects(serde_json::json!({"file_path": "bin"})).await,
        [Effect::Read {
            path: site.real("work/bin"),
            lines: None,
            hash: ContentHash::of(b"a\x00b"),
        }]
    );
    for path in ["dir", "missing.txt"] {
        assert!(
            effects(serde_json::json!({ "file_path": path }))
                .await
                .is_empty(),
            "{path} 没读到文件，不报"
        );
    }
}
