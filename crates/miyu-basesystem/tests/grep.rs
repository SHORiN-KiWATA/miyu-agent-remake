//! `grep`（施工 4-4 下）：三种输出的写法；通配只搜对得上的；`.gitignore` 里的、二进制的不搜；前后行和 `--`；
//! 不分大小写；照 `head_limit`、`offset` 分页；超长的行截掉；没搜到、写错了的说清楚；Claude Code、opencode 的
//! 别名照认。

mod support;

use miyu_kernel::tool::Access;
use miyu_tool::Call;

use support::{Site, native, tool};

/// 照新的在前写出来的几行：`rows` 里的路径是相对工作目录、用 `/` 写的。
fn listed(rows: &[&str]) -> String {
    rows.iter()
        .map(|row| format!("{}\n", native(row)))
        .collect()
}

/// 三个文件：`c.txt` 最新，`b.rs` 里没有，`a.rs` 最旧。
fn three(site: &Site) {
    site.file("work/a.rs", b"fn main() {\n    println!(\"main\");\n}\n");
    site.file("work/b.rs", b"// nothing here\n");
    site.file("work/c.txt", b"the main road\n");
    site.aged("work/a.rs", 300);
    site.aged("work/b.rs", 200);
    site.aged("work/c.txt", 100);
}

#[test]
fn grep_comes_from_the_resources() {
    let tool = tool("grep");
    let spec = tool.spec();
    assert_eq!(spec.access, Access::Read);
    assert!(
        spec.description
            .starts_with("Search file contents with a regular expression"),
        "{}",
        spec.description
    );
    let parameters: serde_json::Value =
        serde_json::from_str(spec.parameters.get()).expect("是 JSON");
    let mut names: Vec<&str> = parameters["properties"]
        .as_object()
        .expect("有 properties")
        .keys()
        .map(String::as_str)
        .collect();
    names.sort_unstable();
    // Claude Code 常用的几个，名字照抄。
    let mut expected = [
        "pattern",
        "path",
        "glob",
        "output_mode",
        "-i",
        "context",
        "head_limit",
        "offset",
    ];
    expected.sort_unstable();
    assert_eq!(names, expected);
    assert_eq!(parameters["required"], serde_json::json!(["pattern"]));
    let targets = tool.targets(&Call {
        args: r#"{"pattern":"x","path":"src"}"#.to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Default::default(),
        stop: Default::default(),
    });
    assert_eq!(targets[0].path, "src");
    assert!(!targets[0].write);
}

#[tokio::test]
async fn by_default_only_the_files_newest_first() {
    let site = Site::new();
    three(&site);
    assert_eq!(
        site.call("grep", serde_json::json!({"pattern": "main"}))
            .await,
        (false, listed(&["c.txt", "a.rs"]))
    );
}

#[tokio::test]
async fn content_shows_path_line_and_text() {
    let site = Site::new();
    three(&site);
    assert_eq!(
        site.call(
            "grep",
            serde_json::json!({"pattern": "main", "output_mode": "content"})
        )
        .await,
        (
            false,
            listed(&[
                "c.txt:1:the main road",
                "a.rs:1:fn main() {",
                "a.rs:2:    println!(\"main\");"
            ])
        )
    );
    assert_eq!(
        site.call(
            "grep",
            serde_json::json!({"pattern": "main", "output_mode": "count"})
        )
        .await,
        (false, listed(&["c.txt:1", "a.rs:2"]))
    );
}

#[tokio::test]
async fn context_lines_and_the_break_between_groups() {
    let site = Site::new();
    let body: String = (1..=10)
        .map(|n| {
            if n == 2 || n == 8 {
                format!("hit {n}\n")
            } else {
                format!("line {n}\n")
            }
        })
        .collect();
    site.file("work/f.txt", body.as_bytes());
    let expected = listed(&[
        "f.txt-1-line 1",
        "f.txt:2:hit 2",
        "f.txt-3-line 3",
        "--",
        "f.txt-7-line 7",
        "f.txt:8:hit 8",
        "f.txt-9-line 9",
    ]);
    for args in [
        serde_json::json!({"pattern": "hit", "output_mode": "content", "context": 1}),
        serde_json::json!({"pattern": "hit", "output_mode": "content", "-C": 1}),
        // `context` 在的时候压过 `-A`、`-B`（Claude Code 的规矩）。
        serde_json::json!({"pattern": "hit", "output_mode": "content", "context": 1, "-A": 3}),
        // 没声明的别名写成字符串的整数，工具自己认（施工 4-9 再补二）。
        serde_json::json!({"pattern": "hit", "output_mode": "content", "-C": "1"}),
        serde_json::json!({"pattern": "hit", "output_mode": "content", "-A": "1", "-B": " 1 "}),
    ] {
        assert_eq!(site.call("grep", args).await, (false, expected.clone()));
    }
    // 只要后面的：`-A`。
    assert_eq!(
        site.call(
            "grep",
            serde_json::json!({"pattern": "hit 8", "output_mode": "content", "-A": 2})
        )
        .await,
        (
            false,
            listed(&["f.txt:8:hit 8", "f.txt-9-line 9", "f.txt-10-line 10"])
        )
    );
}

#[tokio::test]
async fn a_match_left_off_the_page_takes_its_context_lines_with_it() {
    let site = Site::new();
    let body: String = (1..=10)
        .map(|n| {
            if n == 2 || n == 8 {
                format!("hit {n}\n")
            } else {
                format!("line {n}\n")
            }
        })
        .collect();
    site.file("work/f.txt", body.as_bytes());
    // 跳过第一处：它前后的那几行也不写。
    assert_eq!(
        site.call(
            "grep",
            serde_json::json!({"pattern": "hit", "output_mode": "content", "context": 1, "offset": 1})
        )
        .await,
        (
            false,
            listed(&["f.txt-7-line 7", "f.txt:8:hit 8", "f.txt-9-line 9"])
        )
    );
    // 只要一处：后面那一处前后的也不写，说还有。
    assert_eq!(
        site.call(
            "grep",
            serde_json::json!({"pattern": "hit", "output_mode": "content", "context": 1, "head_limit": 1})
        )
        .await,
        (
            false,
            format!(
                "{}(Showing matches 1-1; there are more. Use offset=1 to continue.)\n",
                listed(&["f.txt-1-line 1", "f.txt:2:hit 2", "f.txt-3-line 3"])
            )
        )
    );
}

#[tokio::test]
async fn glob_and_include_pick_the_files_a_named_file_is_searched_as_is() {
    let site = Site::new();
    three(&site);
    site.file("work/src/d.rs", b"main\n");
    site.aged("work/src/d.rs", 400);
    for args in [
        serde_json::json!({"pattern": "main", "glob": "*.rs"}),
        serde_json::json!({"pattern": "main", "include": "*.rs"}),
    ] {
        assert_eq!(
            site.call("grep", args).await,
            (false, listed(&["a.rs", "src/d.rs"]))
        );
    }
    // 点名的一个文件：只搜它，不管通配。
    assert_eq!(
        site.call(
            "grep",
            serde_json::json!({"pattern": "main", "path": "c.txt", "glob": "*.rs"})
        )
        .await,
        (false, listed(&["c.txt"]))
    );
}

#[tokio::test]
async fn ignored_binary_and_the_data_root_are_not_searched() {
    let site = Site::new();
    site.file("work/.gitignore", b"target/\n");
    site.file("work/target/out.txt", b"needle\n");
    site.file("work/app.bin", b"needle\0\x01\x02");
    site.file("work/.env.example", b"needle\n");
    site.file("data/run/token", b"needle\n");
    site.file("work/keep.txt", b"needle\n");
    site.aged("work/keep.txt", 100);
    assert_eq!(
        site.call("grep", serde_json::json!({"pattern": "needle"}))
            .await,
        (false, listed(&[".env.example", "keep.txt"]))
    );
    let root = site.0.to_string_lossy().into_owned();
    let (_, text) = site
        .call(
            "grep",
            serde_json::json!({"pattern": "needle", "path": root}),
        )
        .await;
    assert!(!text.contains("token"), "{text}");
}

#[tokio::test]
async fn case_insensitive_and_utf16() {
    let site = Site::new();
    site.file("work/a.txt", b"Hello World\n");
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("hello there\n".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    site.file("work/b.txt", &utf16);
    site.aged("work/a.txt", 100);
    assert_eq!(
        site.call(
            "grep",
            serde_json::json!({"pattern": "hello", "output_mode": "content"})
        )
        .await,
        (false, listed(&["b.txt:1:hello there"]))
    );
    assert_eq!(
        site.call(
            "grep",
            serde_json::json!({"pattern": "hello", "-i": true, "output_mode": "content"})
        )
        .await,
        (
            false,
            listed(&["b.txt:1:hello there", "a.txt:1:Hello World"])
        )
    );
}

#[tokio::test]
async fn head_limit_and_offset_page_through() {
    let site = Site::new();
    for n in 1..=5 {
        site.file(&format!("work/{n}.txt"), b"x\n");
        site.aged(&format!("work/{n}.txt"), n * 10);
    }
    let call = |args: serde_json::Value| site.call("grep", args);
    assert_eq!(
        call(serde_json::json!({"pattern": "x", "head_limit": 2})).await,
        (
            false,
            format!(
                "{}(Showing files 1-2 of 5. Use offset=2 to continue.)\n",
                listed(&["1.txt", "2.txt"])
            )
        )
    );
    assert_eq!(
        call(serde_json::json!({"pattern": "x", "head_limit": 2, "offset": 2})).await,
        (
            false,
            format!(
                "{}(Showing files 3-4 of 5. Use offset=4 to continue.)\n",
                listed(&["3.txt", "4.txt"])
            )
        )
    );
    assert_eq!(
        call(serde_json::json!({"pattern": "x", "head_limit": 2, "offset": 4})).await,
        (false, listed(&["5.txt"]))
    );
    assert_eq!(
        call(serde_json::json!({"pattern": "x", "offset": 10})).await,
        (
            false,
            "(There are only 5 results; offset 10 skips them all.)\n".to_string()
        )
    );
    // 0 是不限。
    let (_, text) = call(serde_json::json!({"pattern": "x", "head_limit": 0})).await;
    assert_eq!(text.lines().count(), 5);
    // 匹配的行：够数就停，不数一共几条。
    assert_eq!(
        call(serde_json::json!({"pattern": "x", "output_mode": "content", "head_limit": 2})).await,
        (
            false,
            format!(
                "{}(Showing matches 1-2; there are more. Use offset=2 to continue.)\n",
                listed(&["1.txt:1:x", "2.txt:1:x"])
            )
        )
    );
    assert_eq!(
        call(serde_json::json!({"pattern": "x", "output_mode": "content", "offset": 3})).await,
        (false, listed(&["4.txt:1:x", "5.txt:1:x"]))
    );
}

#[tokio::test]
async fn by_default_at_most_two_hundred_fifty() {
    let site = Site::new();
    for n in 0..260 {
        site.file(&format!("work/many/{n:03}.txt"), b"x\n");
    }
    let (_, text) = site.call("grep", serde_json::json!({"pattern": "x"})).await;
    assert_eq!(text.lines().count(), 251);
    assert!(
        text.ends_with("(Showing files 1-250 of 260. Use offset=250 to continue.)\n"),
        "{}",
        &text[text.len() - 80..]
    );
}

#[tokio::test]
async fn a_long_line_is_cut_to_five_hundred() {
    let site = Site::new();
    let long = format!("needle {}\n", "字".repeat(600));
    site.file("work/min.js", long.as_bytes());
    let (_, text) = site
        .call(
            "grep",
            serde_json::json!({"pattern": "needle", "output_mode": "content"}),
        )
        .await;
    let shown = text
        .trim_end()
        .strip_prefix("min.js:1:")
        .expect("路径和行号");
    assert_eq!(shown.chars().count(), 501);
    assert!(shown.ends_with('…'));
}

#[tokio::test]
async fn nothing_found_is_not_an_error_but_a_broken_pattern_is() {
    let site = Site::new();
    three(&site);
    assert_eq!(
        site.call("grep", serde_json::json!({"pattern": "zebra"}))
            .await,
        (false, "No files found\n".to_string())
    );
    for mode in ["content", "count"] {
        assert_eq!(
            site.call(
                "grep",
                serde_json::json!({"pattern": "zebra", "output_mode": mode})
            )
            .await,
            (false, "No matches found\n".to_string())
        );
    }
    assert_eq!(
        site.call("grep", serde_json::json!({"pattern": "(main"}))
            .await,
        (
            true,
            "The pattern is not a valid regular expression: unclosed group.\n".to_string()
        )
    );
    let (error, text) = site
        .call("grep", serde_json::json!({"pattern": "x", "glob": "[ab"}))
        .await;
    assert!(error);
    assert!(
        text.starts_with("The glob \"[ab\" is not valid: "),
        "{text}"
    );
    assert_eq!(
        site.call(
            "grep",
            serde_json::json!({"pattern": "x", "path": "nowhere"})
        )
        .await,
        (
            true,
            "There is no file or directory at \"nowhere\".\n".to_string()
        )
    );
}

/// 点名一个没人写的 FIFO：照「安全地打开」开，马上交回，当没搜到（施工 4-9 再补二：原来普通地打开，一直等着）。在
/// 另一个线程里跑：卡住的话，五秒后打开写的一头把它放出来，再报红，不陪它一直等。
#[cfg(unix)]
#[test]
fn a_named_fifo_is_not_waited_on() {
    let site = Site::new();
    let fifo = site.real("work").join("pipe");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("有 mkfifo");
    assert!(made.success());
    let (tell, heard) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("造得了运行时");
        let args = serde_json::json!({"pattern": "x", "path": "pipe"});
        let got = runtime.block_on(site.call("grep", args));
        tell.send(got).expect("测试还在等");
    });
    match heard.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok((error, _)) => assert!(!error, "没搜到不算出错"),
        Err(_) => {
            let released = std::fs::OpenOptions::new().write(true).open(&fifo);
            panic!("grep 卡在 FIFO 上（放它出来：{:?}）", released.map(|_| ()));
        }
    }
}
