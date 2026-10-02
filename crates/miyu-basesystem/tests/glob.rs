//! `glob`（施工 4-4 下）：模式照 ripgrep 的 `--glob`；遵守 `.gitignore`；新的在前；在工作目录里的写相对路径、
//! 外面的写绝对路径；最多 100 个；找不到、写错了的说清楚。

mod support;

use miyu_kernel::tool::Access;
use miyu_tool::Call;

use support::{Site, absolute, native, tool};

/// 照新的在前写出来的几行：`paths` 是相对工作目录、用 `/` 写的。
fn listed(paths: &[&str]) -> String {
    paths
        .iter()
        .map(|path| format!("{}\n", native(path)))
        .collect()
}

#[test]
fn glob_comes_from_the_resources() {
    let tool = tool("glob");
    let spec = tool.spec();
    assert_eq!(spec.access, Access::Read);
    assert!(
        spec.description.starts_with("Find files by glob pattern"),
        "{}",
        spec.description
    );
    assert_eq!(
        spec.parameters.get(),
        r#"{"type":"object","properties":{"pattern":{"type":"string","description":"A pattern without / matches file names at any depth."},"path":{"type":"string","description":"The directory to search in. Default is the working directory."}},"required":["pattern"]}"#
    );
    let target = |args: &str| {
        tool.targets(&Call {
            args: args.to_string(),
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
        })
    };
    assert_eq!(target(r#"{"pattern":"*.rs"}"#)[0].path, ".");
    assert_eq!(target(r#"{"pattern":"*.rs","path":"src"}"#)[0].path, "src");
    assert_eq!(
        target(r#"{"pattern":"*.rs","path":"undefined"}"#)[0].path,
        ".",
        "写成 undefined 的当没给"
    );
    // 模式本身是绝对路径的：要碰的是它前面那一截目录。
    assert_eq!(
        target(r#"{"pattern":"~/notes/*.md"}"#)[0].path,
        native("~/notes")
    );
    assert!(!target(r#"{"pattern":"*.rs"}"#)[0].write);
}

#[tokio::test]
async fn a_name_pattern_finds_every_level_newest_first() {
    let site = Site::new();
    site.file("work/a.rs", b"");
    site.file("work/src/b.rs", b"");
    site.file("work/src/deep/c.rs", b"");
    site.file("work/readme.md", b"");
    site.aged("work/a.rs", 300);
    site.aged("work/src/b.rs", 200);
    site.aged("work/src/deep/c.rs", 100);
    assert_eq!(
        site.call("glob", serde_json::json!({"pattern": "*.rs"}))
            .await,
        (false, listed(&["src/deep/c.rs", "src/b.rs", "a.rs"]))
    );
    // 带 `/` 的照路径比：`*` 不跨目录，`**` 跨。
    assert_eq!(
        site.call("glob", serde_json::json!({"pattern": "src/*.rs"}))
            .await,
        (false, listed(&["src/b.rs"]))
    );
    assert_eq!(
        site.call("glob", serde_json::json!({"pattern": "src/**/*.rs"}))
            .await,
        (false, listed(&["src/deep/c.rs", "src/b.rs"]))
    );
    assert_eq!(
        site.call(
            "glob",
            serde_json::json!({"pattern": "*.{md,rs}", "path": "src"})
        )
        .await,
        (false, listed(&["src/deep/c.rs", "src/b.rs"])),
        "path 下面照样写相对工作目录的路径"
    );
}

#[tokio::test]
async fn the_same_time_sorts_by_path() {
    let site = Site::new();
    for name in ["c.txt", "a.txt", "b.txt"] {
        site.file(&format!("work/{name}"), b"");
        site.aged(&format!("work/{name}"), 60);
    }
    assert_eq!(
        site.call("glob", serde_json::json!({"pattern": "*.txt"}))
            .await,
        (false, listed(&["a.txt", "b.txt", "c.txt"]))
    );
}

#[tokio::test]
async fn gitignore_counts_even_outside_a_repository() {
    let site = Site::new();
    site.file("work/.gitignore", b"target/\n*.tmp\n");
    site.file("work/keep.rs", b"");
    site.file("work/junk.tmp", b"");
    site.file("work/target/built.rs", b"");
    site.file("work/.ignore", b"vendor/\n");
    site.file("work/vendor/v.rs", b"");
    site.aged("work/keep.rs", 10);
    assert_eq!(
        site.call("glob", serde_json::json!({"pattern": "*.rs"}))
            .await,
        (false, listed(&["keep.rs"]))
    );
}

#[tokio::test]
async fn in_a_repository_the_ignore_files_above_count_outside_they_do_not() {
    let site = Site::new();
    // 在仓库里：仓库根的 .gitignore 管到下面的子目录。
    std::fs::create_dir_all(site.0.join("work/.git")).expect("建得了");
    site.file("work/.gitignore", b"build/\n");
    site.file("work/sub/build/x.rs", b"");
    site.file("work/sub/y.rs", b"");
    assert_eq!(
        site.call(
            "glob",
            serde_json::json!({"pattern": "*.rs", "path": "sub"})
        )
        .await,
        (false, listed(&["sub/y.rs"]))
    );
    // 不在仓库里：上面那一层写着 `*` 的 .gitignore 不管搜的这个目录。
    site.file("home/.gitignore", b"*\n");
    site.file("home/notes/z.md", b"");
    let (error, text) = site
        .call(
            "glob",
            serde_json::json!({"pattern": "*.md", "path": "~/notes"}),
        )
        .await;
    assert!(!error);
    assert_eq!(
        text,
        format!("{}\n", absolute(&site.real("home/notes/z.md")))
    );
}

#[tokio::test]
async fn hidden_files_are_found_but_not_the_repository_itself() {
    let site = Site::new();
    site.file("work/.github/ci.yml", b"");
    site.file("work/.git/config.yml", b"");
    site.file("work/.hg/store.yml", b"");
    assert_eq!(
        site.call("glob", serde_json::json!({"pattern": "*.yml"}))
            .await,
        (false, listed(&[".github/ci.yml"]))
    );
}

#[tokio::test]
async fn outside_the_working_directory_paths_are_absolute_and_the_data_root_is_skipped() {
    let site = Site::new();
    site.file("data/run/token.txt", b"secret");
    site.file("home/plan.txt", b"");
    site.file("work/w.txt", b"");
    site.aged("work/w.txt", 100);
    let root = site.0.to_string_lossy().into_owned();
    let (error, text) = site
        .call(
            "glob",
            serde_json::json!({"pattern": "*.txt", "path": root}),
        )
        .await;
    assert!(!error);
    // 从场地的根搜下来：工作目录里的写相对路径，外面的写绝对路径，数据根里的一个都不列。
    assert_eq!(
        text,
        format!(
            "{}\n{}\n",
            absolute(&site.real("home/plan.txt")),
            native("w.txt")
        )
    );
}

#[tokio::test]
async fn a_workspace_inside_the_data_root_is_searched_all_the_way_down() {
    let site = Site::new();
    // 账号自己的工作区就在数据根里：照样往下走，和权限策略的先后一样；数据根别的地方照样不进。
    site.file("data/home/admin/workspace/src/deep/a.rs", b"");
    site.file("data/home/admin/workspace/b.rs", b"");
    site.file("data/state/c.rs", b"");
    site.aged("data/home/admin/workspace/b.rs", 100);
    assert_eq!(
        site.call_in(
            "data/home/admin/workspace",
            "glob",
            serde_json::json!({"pattern": "*.rs"})
        )
        .await,
        (false, listed(&["src/deep/a.rs", "b.rs"]))
    );
    let root = site.0.join("data").to_string_lossy().into_owned();
    let (error, text) = site
        .call_in(
            "data/home/admin/workspace",
            "glob",
            serde_json::json!({"pattern": "*.rs", "path": root}),
        )
        .await;
    assert!(!error);
    assert!(!text.contains("c.rs"), "{text}");
}

#[tokio::test]
async fn an_absolute_pattern_searches_from_its_directory() {
    let site = Site::new();
    site.file("work/src/a.rs", b"");
    site.file("work/src/deep/b.rs", b"");
    site.file("work/Cargo.toml", b"");
    site.file("work/src/Cargo.toml", b"");
    site.aged("work/src/a.rs", 100);
    let pattern = site.real("work/src").join("**").join("*.rs");
    assert_eq!(
        site.call(
            "glob",
            serde_json::json!({"pattern": pattern.to_string_lossy()})
        )
        .await,
        (false, listed(&["src/deep/b.rs", "src/a.rs"]))
    );
    // 一个通配都没有的：只认那一层的那个文件，下面几层同名的不算。
    let exact = site.real("work/Cargo.toml");
    assert_eq!(
        site.call(
            "glob",
            serde_json::json!({"pattern": exact.to_string_lossy()})
        )
        .await,
        (false, listed(&["Cargo.toml"]))
    );
}

#[tokio::test]
async fn more_than_a_hundred_says_how_many_are_left() {
    let site = Site::new();
    for n in 0..103 {
        site.file(&format!("work/many/{n:03}.txt"), b"");
    }
    let (error, text) = site
        .call("glob", serde_json::json!({"pattern": "*.txt"}))
        .await;
    assert!(!error);
    assert_eq!(text.lines().count(), 101);
    assert!(
        text.ends_with("(Showing 100 of 103 matching files; 3 more are not listed. Narrow the pattern or path to see the rest.)\n"),
        "{}",
        &text[text.len() - 120..]
    );
}

#[tokio::test]
async fn nothing_found_is_not_an_error_but_a_broken_pattern_is() {
    let site = Site::new();
    site.file("work/a.rs", b"");
    assert_eq!(
        site.call("glob", serde_json::json!({"pattern": "*.py"}))
            .await,
        (false, "No files found\n".to_string())
    );
    let (error, text) = site
        .call("glob", serde_json::json!({"pattern": "[ab"}))
        .await;
    assert!(error);
    assert!(
        text.starts_with("The glob \"[ab\" is not valid: unclosed character class"),
        "{text}"
    );
}

#[tokio::test]
async fn the_path_must_be_a_directory_that_exists() {
    let site = Site::new();
    site.file("work/a.rs", b"");
    site.file("work/sources/x.rs", b"");
    assert_eq!(
        site.call(
            "glob",
            serde_json::json!({"pattern": "*.rs", "path": "a.rs"})
        )
        .await,
        (true, "\"a.rs\" is not a directory.\n".to_string())
    );
    assert_eq!(
        site.call(
            "glob",
            serde_json::json!({"pattern": "*.rs", "path": "sorces"})
        )
        .await,
        (
            true,
            "There is no file or directory at \"sorces\".\nDid you mean \"sources\"?\n".to_string()
        )
    );
    let (error, text) = site.call("glob", serde_json::json!({"path": "."})).await;
    assert!(error);
    assert!(text.starts_with("The arguments are not right: "), "{text}");
}
