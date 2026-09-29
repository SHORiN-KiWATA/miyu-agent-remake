//! `trash`（施工 4-6 下）：不许删的几种、不存在的；Linux 上放进家目录的回收站，`.trashinfo` 写着原来的位置和时间，
//! 重名的接 `.2`，目录、链接，路径照规范转义，挪不动的、家目录的回收站建不了的不删；macOS、Windows 上删文件、删目录
//! （在 CI 上跑）。

mod support;

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_kernel::event::Said;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Effect};

use support::{Site, tool};

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

/// 在 `work/` 里删 `file_path`。
async fn trash(site: &Site, file_path: &str) -> Done {
    site.done("trash", json!({ "file_path": file_path })).await
}

#[test]
fn trash_comes_from_the_resources_and_names_what_it_writes() {
    let tool = tool("trash");
    assert_eq!(tool.spec().access, Access::Write);
    // 写成 `path` 的也认。
    for args in [json!({"file_path": "old.txt"}), json!({"path": "old.txt"})] {
        let targets = tool.targets(&Call {
            args: args.to_string(),
            cwd: String::new(),
            home: None,
            data_root: None,
            seen: Default::default(),
            stop: Default::default(),
            sandbox: None,
        });
        assert_eq!(targets.len(), 1, "{args}");
        assert_eq!(targets[0].path, "old.txt");
        assert!(targets[0].write);
    }
}

#[tokio::test]
async fn the_working_directory_its_parents_home_and_root_are_not_deleted() {
    let site = Site::new();
    let work = site.real("work").to_string_lossy().into_owned();
    let above = site.real("").to_string_lossy().into_owned();
    let home = site.real("home").to_string_lossy().into_owned();
    for path in [
        ".",
        "..",
        "~",
        "~/",
        "~/.",
        "/",
        work.as_str(),
        above.as_str(),
        home.as_str(),
    ] {
        let done = trash(&site, path).await;
        assert!(done.error, "{path}");
        assert_eq!(
            done.human,
            Some(said("trash/protected")),
            "{path}: {}",
            text(&done)
        );
    }
    assert!(
        site.0.join("work").is_dir() && site.0.join("home").is_dir(),
        "都还在"
    );
    let done = trash(&site, "nope.txt").await;
    assert!(done.error);
    assert!(
        matches!(&done.human, Some(said) if said.key.ends_with("common/missing")),
        "{:?}",
        done.human
    );
}

#[cfg(target_os = "linux")]
mod linux {
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

    use miyu_tool::Progress;

    use super::*;
    use crate::support::native;

    /// 场地里假家目录的回收站。
    fn bin(site: &Site) -> std::path::PathBuf {
        site.0.join("home/.local/share/Trash")
    }

    #[tokio::test]
    async fn a_file_goes_into_the_home_trash_with_its_record() {
        let site = Site::new();
        site.file("work/a.txt", b"old\n");
        let real = site.real("work").join("a.txt");
        let done = trash(&site, "a.txt").await;
        assert!(!done.error, "{}", text(&done));
        let quoted = serde_json::to_string(&native("a.txt")).expect("JSON");
        assert_eq!(text(&done), format!("Moved {quoted} to the trash.\n"));
        assert!(!real.exists(), "不在原处了");
        let kept = bin(&site).join("files/a.txt");
        assert_eq!(std::fs::read(&kept).expect("在回收站里"), b"old\n");
        let record =
            std::fs::read_to_string(bin(&site).join("info/a.txt.trashinfo")).expect("有记录");
        let lines: Vec<&str> = record.lines().collect();
        assert_eq!(lines[0], "[Trash Info]");
        assert_eq!(lines[1], format!("Path={}", real.display()));
        let date = lines[2].strip_prefix("DeletionDate=").expect("有删的时间");
        assert_eq!(date.len(), 19, "{date}");
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[10..11], "T");
        assert_eq!(done.human, Some(said("trash/trashed")));
        assert_eq!(
            done.effects,
            [Effect::Trashed {
                path: real,
                trash: kept.to_string_lossy().into_owned(),
            }]
        );
        for dir in ["files", "info"] {
            let meta = std::fs::metadata(bin(&site).join(dir)).expect("建了");
            assert_eq!(
                meta.permissions().mode() & 0o777,
                0o700,
                "{dir} 只有自己能进"
            );
        }
    }

    #[tokio::test]
    async fn the_same_name_again_gets_a_number() {
        let site = Site::new();
        for round in 0..2 {
            site.file("work/a.txt", format!("round {round}\n").as_bytes());
            let done = trash(&site, "a.txt").await;
            assert!(!done.error, "{}", text(&done));
        }
        assert_eq!(
            std::fs::read(bin(&site).join("files/a.txt")).expect("第一个"),
            b"round 0\n"
        );
        assert_eq!(
            std::fs::read(bin(&site).join("files/a.txt.2")).expect("第二个"),
            b"round 1\n"
        );
        assert!(bin(&site).join("info/a.txt.2.trashinfo").is_file());
        assert!(
            bin(&site).join("info/a.txt.trashinfo").is_file(),
            "第一个的记录还在"
        );
    }

    /// 回收站里有个同名的、却没有记录（别的程序留下的）：不盖掉它，接 `.2`，没用上的那份记录删掉。
    #[tokio::test]
    async fn a_name_taken_without_a_record_is_not_overwritten() {
        let site = Site::new();
        site.file("home/.local/share/Trash/files/a.txt", b"left\n");
        site.file("work/a.txt", b"new\n");
        let done = trash(&site, "a.txt").await;
        assert!(!done.error, "{}", text(&done));
        assert_eq!(
            std::fs::read(bin(&site).join("files/a.txt")).expect("还在"),
            b"left\n"
        );
        assert_eq!(
            std::fs::read(bin(&site).join("files/a.txt.2")).expect("接了号"),
            b"new\n"
        );
        assert!(!bin(&site).join("info/a.txt.trashinfo").exists());
        assert!(bin(&site).join("info/a.txt.2.trashinfo").is_file());
    }

    #[tokio::test]
    async fn directories_and_links_go_as_they_are() {
        let site = Site::new();
        site.file("work/dir/inner.txt", b"in\n");
        let done = trash(&site, "dir").await;
        assert!(!done.error, "{}", text(&done));
        assert!(!site.0.join("work/dir").exists());
        assert_eq!(
            std::fs::read(bin(&site).join("files/dir/inner.txt")).expect("整个目录进去了"),
            b"in\n"
        );
        site.file("work/target.txt", b"keep\n");
        symlink(site.0.join("work/target.txt"), site.0.join("work/link")).expect("建得了链接");
        let done = trash(&site, "link").await;
        assert!(!done.error, "{}", text(&done));
        assert!(
            std::fs::symlink_metadata(site.0.join("work/link")).is_err(),
            "链接没了"
        );
        assert_eq!(
            std::fs::read(site.0.join("work/target.txt")).expect("指向的还在"),
            b"keep\n"
        );
        assert!(
            std::fs::symlink_metadata(bin(&site).join("files/link"))
                .expect("在回收站里")
                .file_type()
                .is_symlink()
        );
        // 指向的东西没了的链接也删得掉。
        symlink(site.0.join("work/gone.txt"), site.0.join("work/dangling")).expect("建得了链接");
        let done = trash(&site, "dangling").await;
        assert!(!done.error, "{}", text(&done));
        assert!(std::fs::symlink_metadata(site.0.join("work/dangling")).is_err());
    }

    #[tokio::test]
    async fn the_original_path_is_escaped_in_the_record() {
        let site = Site::new();
        site.file("work/a b 中.txt", b"x");
        let done = trash(&site, "a b 中.txt").await;
        assert!(!done.error, "{}", text(&done));
        let record =
            std::fs::read_to_string(bin(&site).join("info/a b 中.txt.trashinfo")).expect("有记录");
        assert!(record.contains("/a%20b%20%E4%B8%AD.txt\n"), "{record}");
    }

    /// 挪不动的（所在的目录只读）：不删，说系统的原话，那份 `.trashinfo` 也删掉。root 不受权限管，跳过。
    #[tokio::test]
    async fn what_cannot_be_moved_stays_and_leaves_no_record() {
        if std::fs::metadata("/proc/self").expect("有").uid() == 0 {
            return;
        }
        let site = Site::new();
        site.file("work/locked/a.txt", b"keep\n");
        let locked = site.0.join("work/locked");
        let mode = |mode| std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(mode));
        mode(0o555).expect("改得了权限");
        let done = trash(&site, "locked/a.txt").await;
        mode(0o755).expect("改得回来");
        assert!(done.error, "{}", text(&done));
        assert_eq!(
            done.human,
            Some(said("trash/failed").with("error", "Permission denied (os error 13)")),
            "{}",
            text(&done)
        );
        assert_eq!(
            std::fs::read(locked.join("a.txt")).expect("还在"),
            b"keep\n"
        );
        let records = std::fs::read_dir(bin(&site).join("info"))
            .expect("建了")
            .count();
        assert_eq!(records, 0, "没留下记录");
    }

    /// 交给工具的家目录是个链接：照它指向的真实位置拦。
    #[tokio::test]
    async fn a_home_given_through_a_link_is_still_protected() {
        let site = Site::new();
        symlink(site.0.join("home"), site.0.join("home-link")).expect("建得了链接");
        let call = Call {
            args: json!({ "file_path": site.real("home") }).to_string(),
            cwd: site.0.join("work").to_string_lossy().into_owned(),
            home: Some(site.0.join("home-link")),
            data_root: None,
            seen: Default::default(),
            stop: Default::default(),
            sandbox: None,
        };
        let done = tool("trash").run(call, Progress::new(|_| {})).await;
        assert_eq!(done.human, Some(said("trash/protected")), "{}", text(&done));
        assert!(site.0.join("home").is_dir());
    }

    #[tokio::test]
    async fn without_a_home_trash_nothing_is_deleted() {
        let site = Site::new();
        // 家目录的回收站该在的地方被一个文件占了：建不起来。
        site.file("home/.local", b"not a directory");
        site.file("work/a.txt", b"keep\n");
        let done = trash(&site, "a.txt").await;
        assert!(done.error, "{}", text(&done));
        assert_eq!(done.human, Some(said("trash/unavailable")));
        assert_eq!(
            std::fs::read(site.0.join("work/a.txt")).expect("还在"),
            b"keep\n"
        );
    }
}

#[cfg(any(target_os = "macos", windows))]
#[tokio::test]
async fn files_and_directories_go_into_the_system_trash() {
    let site = Site::new();
    site.file("work/a.txt", b"old\n");
    site.file("work/dir/inner.txt", b"in\n");
    for name in ["a.txt", "dir"] {
        let done = trash(&site, name).await;
        assert!(!done.error, "{name}: {}", text(&done));
        assert!(
            !site.0.join("work").join(name).exists(),
            "{name} 不在原处了"
        );
        match &done.effects[..] {
            // 记下的是回收站里的真实位置：真有这个东西。
            [Effect::Trashed { trash, .. }] => {
                assert!(std::path::Path::new(trash).exists(), "{name}: {trash}");
            }
            other => panic!("{other:?}"),
        }
    }
}

/// macOS、Windows 上换个大小写写的工作目录、家目录，照样不许删（施工 4-9 再补二）：最后一段是她写的原样，比的时候
/// 不分大小写。
#[cfg(any(target_os = "macos", windows))]
#[tokio::test]
async fn a_different_case_is_still_protected() {
    let site = Site::new();
    let home = site.0.join("HOME").to_string_lossy().into_owned();
    for path in ["../WORK", "../Work/.", home.as_str()] {
        let done = site.done("trash", json!({ "file_path": path })).await;
        assert_eq!(
            done.human,
            Some(said("trash/protected")),
            "{path}：{}",
            text(&done)
        );
    }
    assert!(site.0.join("work").is_dir() && site.0.join("home").is_dir());
}
