//! 按目录找：开头对、大小写不论、点开头的打了点才列、目录在前、50 条截断、`partial`；落进边界表「谁都不能碰」
//! 那一片的不列（`web-module.md`「怎么走」第 2、3 条）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::boundary::{Boundary, Places};

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-list-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // 换成真实的位置：macOS 上系统的临时目录在 `/var` 下，它是个链接。
        Scratch(fs::canonicalize(dir).unwrap())
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 建一份文件、目录：`relative` 以 `/` 结尾的是目录。
fn touch(root: &Path, relative: &str) {
    let path = root.join(relative);
    if relative.ends_with('/') {
        fs::create_dir_all(&path).expect("建得了目录");
    } else {
        fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        fs::write(&path, "x").expect("写得进");
    }
}

/// 不挡任何路径的边界表：工作区就是这个目录本身，数据根指到一个不存在的地方。
fn permissive(root: &Path) -> Boundary {
    let places = Places::here(root.to_path_buf(), root.join("does-not-exist"), None);
    Boundary::new(&places)
}

fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.path.as_str()).collect()
}

#[test]
fn names_are_filtered_by_prefix_case_insensitively_dirs_come_first() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    for p in ["b.txt", "a/", "Abc.md", ".hidden", "zeta/", "ab.rs"] {
        touch(root, p);
    }
    let boundary = permissive(root);

    let (all, partial) = list_dir(root, "", &boundary).expect("列得了");
    assert_eq!(
        names(&all),
        ["a/", "zeta/", "ab.rs", "Abc.md", "b.txt"],
        "目录在前，各照名字排（大小写不论）"
    );
    assert!(!partial);

    let (a, _) = list_dir(root, "a", &boundary).expect("列得了");
    assert_eq!(names(&a), ["a/", "ab.rs", "Abc.md"], "开头对、大小写不论");

    let full = &all[2].full;
    assert!(full.ends_with("ab.rs") && full.is_absolute());
}

#[test]
fn hidden_names_only_show_when_the_prefix_also_starts_with_a_dot() {
    let scratch = Scratch::new();
    touch(&scratch.0, ".hidden");
    touch(&scratch.0, "shown.txt");
    let boundary = permissive(&scratch.0);

    let (all, _) = list_dir(&scratch.0, "", &boundary).expect("列得了");
    assert_eq!(names(&all), ["shown.txt"], "没打点的不列点开头的");

    let (dot, _) = list_dir(&scratch.0, ".", &boundary).expect("列得了");
    assert_eq!(names(&dot), [".hidden"], "打了点才列");
}

#[test]
fn more_than_fifty_entries_are_truncated_and_marked_partial() {
    let scratch = Scratch::new();
    for i in 0..60 {
        touch(&scratch.0, &format!("f{i:02}.txt"));
    }
    let boundary = permissive(&scratch.0);

    let (entries, partial) = list_dir(&scratch.0, "", &boundary).expect("列得了");
    assert_eq!(entries.len(), 50, "最多 50 条");
    assert!(partial, "多了截掉要说 partial");
}

#[test]
fn exactly_fifty_entries_are_not_marked_partial() {
    let scratch = Scratch::new();
    for i in 0..50 {
        touch(&scratch.0, &format!("f{i:02}.txt"));
    }
    let boundary = permissive(&scratch.0);

    let (entries, partial) = list_dir(&scratch.0, "", &boundary).expect("列得了");
    assert_eq!(entries.len(), 50);
    assert!(!partial, "刚好 50 条，没有多出来的，不该说 partial");
}

#[test]
fn entries_in_the_forbidden_zone_are_skipped_but_siblings_still_list() {
    let scratch = Scratch::new();
    touch(&scratch.0, "data/secret.txt");
    touch(&scratch.0, "keep.txt");
    // 工作区在别处，数据根是 `data/`：`data/` 这一条自己落进「谁都不能碰」那一片。
    let places = Places::here(scratch.0.join("elsewhere"), scratch.0.join("data"), None);
    let boundary = Boundary::new(&places);

    let (entries, _) = list_dir(&scratch.0, "", &boundary).expect("列得了");
    assert_eq!(
        names(&entries),
        ["keep.txt"],
        "数据根本身也不列，旁边的照样列"
    );
}

#[test]
fn an_unreadable_directory_is_an_error() {
    let scratch = Scratch::new();
    let boundary = permissive(&scratch.0);
    assert!(list_dir(&scratch.0.join("does-not-exist"), "", &boundary).is_err());
}
