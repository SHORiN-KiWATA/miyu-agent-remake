//! `read`（施工 4-4 上）：从资源目录造出来；读文件带行号、翻页；读目录；读不了的说清楚。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::block::Block;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Progress, Tool};

/// 源码树里的资源目录。
fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 一个用完就删的临时目录：假的家 `home/`、工作区 `work/`。
struct Site(PathBuf);

impl Site {
    fn new() -> Site {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-read-{}-{n}", std::process::id()));
        for sub in ["home", "work"] {
            std::fs::create_dir_all(dir.join(sub)).expect("建得了目录");
        }
        Site(dir)
    }

    fn file(&self, path: &str, bytes: &[u8]) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, bytes).expect("写得进");
    }

    /// 在工作区里调一次 `read`，参数是 `args`。
    async fn read(&self, args: serde_json::Value) -> (bool, String) {
        let tool = tool();
        let call = Call {
            args: args.to_string(),
            cwd: self.0.join("work").to_string_lossy().into_owned(),
            home: Some(self.0.join("home")),
        };
        let Done { error, blocks } = tool.run(call, Progress::new(|_| {})).await;
        let text = blocks
            .iter()
            .map(|block| match block {
                Block::Text(text) => text.text.clone(),
                other => panic!("只该有字：{other:?}"),
            })
            .collect();
        (error, text)
    }
}

impl Drop for Site {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 从资源目录造出来的 `read`。
fn tool() -> Arc<dyn Tool> {
    let tools = miyu_basesystem::tools(&resources()).expect("资源目录里的字读得出来");
    tools
        .into_iter()
        .find(|tool| tool.spec().name == "read")
        .expect("有 read")
}

#[test]
fn read_comes_from_the_resources_with_its_schema_as_written() {
    let tool = tool();
    let spec = tool.spec();
    assert_eq!(spec.access, Access::Read);
    assert!(
        spec.description
            .starts_with("Read a text file by line pages"),
        "{}",
        spec.description
    );
    assert_eq!(
        spec.parameters.get(),
        r#"{"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}"#
    );
    let targets = tool.targets(&Call {
        args: r#"{"path":"src/a.rs"}"#.to_string(),
        cwd: String::new(),
        home: None,
    });
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].path, "src/a.rs");
    assert!(!targets[0].write);
}

#[test]
fn a_broken_resource_is_named() {
    let site = Site::new();
    // 只有 read.json、没有输出里的几句的资源目录：说是缺的那一份。
    let broken = site.0.join("resources");
    let tools = broken.join("software").join("basesystem").join("tools");
    std::fs::create_dir_all(&tools).expect("建得了目录");
    std::fs::copy(
        resources().join("software/basesystem/tools/read.json"),
        tools.join("read.json"),
    )
    .expect("拷得了");
    let Err(error) = miyu_basesystem::tools(&broken) else {
        panic!("读不全");
    };
    assert!(error.file.ends_with("more.txt"), "{error}");
    // 说明写坏了：说是哪一份。
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
    let (error, text) = site.read(serde_json::json!({"path": "notes.txt"})).await;
    assert!(!error);
    assert_eq!(
        text,
        body.lines()
            .enumerate()
            .map(|(i, line)| format!("{:>6}\t{line}\n", i + 1))
            .collect::<String>()
    );
    let (_, text) = site
        .read(serde_json::json!({"path": "notes.txt", "offset": 2, "limit": 2}))
        .await;
    assert_eq!(
        text,
        "     2\tline 2\n     3\tline 3\n(Showing lines 2-3 of 5. Use offset=4 to read on.)\n"
    );
    let (error, text) = site
        .read(serde_json::json!({"path": "notes.txt", "offset": 9}))
        .await;
    assert!(!error);
    assert_eq!(text, "(The file has 5 lines; offset 9 is past the end.)\n");
}

#[tokio::test]
async fn home_and_empty_and_binary() {
    let site = Site::new();
    site.file("home/plan.md", b"# plan\n");
    let (error, text) = site.read(serde_json::json!({"path": "~/plan.md"})).await;
    assert!(!error);
    assert_eq!(text, "     1\t# plan\n");
    site.file("work/empty.txt", b"");
    assert_eq!(
        site.read(serde_json::json!({"path": "empty.txt"})).await,
        (false, "(It is empty.)\n".to_string())
    );
    site.file("work/app.bin", b"\x7fELF\0\0");
    assert_eq!(
        site.read(serde_json::json!({"path": "app.bin"})).await,
        (true, "\"app.bin\" is a binary file.\n".to_string())
    );
}

#[tokio::test]
async fn a_directory_lists_its_entries_by_name() {
    let site = Site::new();
    site.file("work/b.txt", b"b");
    site.file("work/a/inner.txt", b"a");
    site.file("work/c.rs", b"c");
    let (error, text) = site.read(serde_json::json!({"path": "."})).await;
    assert!(!error);
    assert_eq!(text, "a/\nb.txt\nc.rs\n");
    std::fs::create_dir_all(site.0.join("work/hollow")).expect("建得了");
    assert_eq!(
        site.read(serde_json::json!({"path": "hollow"})).await,
        (false, "(It is empty.)\n".to_string())
    );
    // 超过 1000 项：列前 1000 项，说还有多少。
    for n in 0..1003 {
        site.file(&format!("work/many/{n:04}.txt"), b"x");
    }
    let (_, text) = site.read(serde_json::json!({"path": "many"})).await;
    assert_eq!(text.lines().count(), 1001);
    assert!(
        text.ends_with("(... and 3 more entries.)\n"),
        "{}",
        &text[text.len() - 60..]
    );
}

#[tokio::test]
async fn what_cannot_be_read_says_so() {
    let site = Site::new();
    assert_eq!(
        site.read(serde_json::json!({"path": "nope.txt"})).await,
        (
            true,
            "There is no file or directory at \"nope.txt\".\n".to_string()
        )
    );
    let (error, text) = site.read(serde_json::json!({"offset": 1})).await;
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
            site.read(serde_json::json!({"path": "pipe"})).await,
            (
                true,
                "\"pipe\" is not a regular file or a directory.\n".to_string()
            )
        );
    }
}

#[tokio::test]
async fn one_read_is_at_most_two_thousand_lines() {
    let site = Site::new();
    let body: String = (1..=2500).map(|n| format!("{n}\n")).collect();
    site.file("work/long.txt", body.as_bytes());
    for args in [
        serde_json::json!({"path": "long.txt"}),
        serde_json::json!({"path": "long.txt", "limit": 5000}),
    ] {
        let (_, text) = site.read(args).await;
        assert_eq!(text.lines().count(), 2001, "2000 行加一句还没读完");
        assert!(
            text.ends_with("(Showing lines 1-2000 of 2500. Use offset=2001 to read on.)\n"),
            "{}",
            &text[text.len() - 80..]
        );
    }
}
