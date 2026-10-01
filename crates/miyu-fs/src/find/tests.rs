//! 打分（照 proto/web-demo 分支 `web-demo/bridge/src/mention.rs` 的 `score`，测试搬过来）；模糊找：认
//! `.gitignore`（不要求是 git 仓库）、跳过隐藏目录和名单、最深几层、收满就停、落进边界表「谁都不能碰」那一片
//! 的不收（`web-module.md`「怎么走」第 2、4 条）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::boundary::{Boundary, Places};

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-find-{}-{n}", std::process::id()));
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

/// 等到清单建完，最多五秒：建清单在后台线程里，别写成死等。
fn built(index: &Index) -> Built {
    let until = Instant::now() + Duration::from_secs(5);
    while !index.with(|built| built.done) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    index.with(|built| Built {
        entries: built.entries.clone(),
        done: built.done,
        partial: built.partial,
    })
}

fn paths(built: &Built) -> Vec<&str> {
    let mut names: Vec<&str> = built
        .entries
        .iter()
        .map(|found| found.path.as_str())
        .collect();
    names.sort_unstable();
    names
}

fn no_errors(_: String) {}

/// 钉死分的算法，不只比相对大小：每个字 1 分、落在文件名里 3 分、一段开头 8 分、连着 5 分、正好是扩展名前的
/// 整个字 100 分，这几个系数任何一个改了都要红（相对大小比较可能照样成立，盯不住系数本身）。
#[test]
fn the_scoring_formula_is_pinned_to_exact_numbers() {
    // "main.rs" 对 "main"：m(0,段开头+8)a(1,连着+5)i(2,连着+5)n(3,连着+5)，4 个字都在文件名里（+3*4=12），
    // 基础 4 分，扩展名前正好是 "main"（+100）。4+12+8+15+100 = 139。
    assert_eq!(score("main.rs", "main"), Some((139, vec![0, 1, 2, 3])));
    // "amainb.rs"：同上但不占开头（前一个字是 "a"，不是分隔符），也不是扩展名前的整个字：
    // 4+12+0+15+0 = 31。
    assert_eq!(score("amainb.rs", "main"), Some((31, vec![1, 2, 3, 4])));
    // "m1a2i3n4.rs"：四个字都对得上、都在文件名里，但谁都不连着、只有开头那个字算段开头：
    // 4+12+8+0+0 = 24。
    assert_eq!(score("m1a2i3n4.rs", "main"), Some((24, vec![0, 2, 4, 6])));
}

#[test]
fn query_characters_must_all_appear_in_order_name_hits_segment_starts_and_runs_score_higher() {
    assert!(score("src/lib.rs", "xyz").is_none());
    assert!(score("src/lib.rs", "bil").is_none(), "要照先后");

    let (_, marks) = score("src/Main.rs", "main").expect("对得上");
    assert_eq!(marks, vec![4, 5, 6, 7], "落在文件名里、不论大小写");

    let exact = score("src/main.rs", "main").expect("对得上").0;
    let inside = score("src/amainb.rs", "main").expect("对得上").0;
    let loose = score("src/amxaxixnb.rs", "main").expect("对得上").0;
    assert!(exact > inside, "文件名去掉扩展名正好是的分高");
    assert!(inside > loose, "连着的分高");

    let named = score("docs/main/x.rs", "main").expect("对得上").0;
    assert!(inside > named, "落在文件名里的分高");

    let start = score("src/web_demo.rs", "demo").expect("对得上").0;
    let middle = score("src/webdemox.rs", "demo").expect("对得上").0;
    assert!(start > middle, "一段开头的分高");

    assert_eq!(
        score("src/main.rs", ""),
        Some((0, Vec::new())),
        "空的都对得上、0 分"
    );
}

#[test]
fn gitignore_is_honored_without_a_git_repository() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    for f in ["src/main.rs", "out/main.log"] {
        touch(root, f);
    }
    fs::write(root.join(".gitignore"), "out/\n").expect("写得进");
    // 没有 `.git`：不要求是 git 仓库也照样认 `.gitignore`。
    assert!(!root.join(".git").exists());

    let index = Index::start(root.clone(), permissive(root), CAP, no_errors);
    let built = built(&index);
    assert!(built.done);
    assert!(
        paths(&built).contains(&"src/main.rs"),
        "{:?}",
        paths(&built)
    );
    assert!(
        !paths(&built).iter().any(|p| p.starts_with("out")),
        "{:?}",
        paths(&built)
    );
}

#[test]
fn hidden_entries_and_the_factory_skip_list_stay_out() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    for f in [
        ".hidden/x.txt",
        ".dotfile",
        "node_modules/pkg/a.js",
        "target/debug/out",
        "keep.md",
    ] {
        touch(root, f);
    }

    let index = Index::start(root.clone(), permissive(root), CAP, no_errors);
    let built = built(&index);
    assert_eq!(paths(&built), ["keep.md"], "隐藏的、出厂名单里的都不收");
}

#[test]
fn depth_is_capped_at_the_configured_limit() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    let mut relative = String::new();
    for i in 0..(DEPTH + 2) {
        relative.push_str(&format!("d{i}/"));
    }
    touch(root, &format!("{relative}deep.txt"));
    touch(root, "shallow.txt");

    let index = Index::start(root.clone(), permissive(root), CAP, no_errors);
    let built = built(&index);
    assert!(paths(&built).contains(&"shallow.txt"));
    assert!(
        !paths(&built).iter().any(|p| p.contains("deep.txt")),
        "超过 {DEPTH} 层的不收：{:?}",
        paths(&built)
    );
}

#[test]
fn the_walk_stops_once_the_cap_is_reached_and_marks_partial() {
    // `cap` 是传给 `Index::start` 的，不用真造两万个文件就能测生产的上限 `CAP`。
    let scratch = Scratch::new();
    let root = &scratch.0;
    for i in 0..10 {
        touch(root, &format!("f{i}.txt"));
    }

    let index = Index::start(root.clone(), permissive(root), 5, no_errors);
    let built = built(&index);
    assert_eq!(built.entries.len(), 5);
    assert!(built.partial, "收满就停，记着只是一部分");
}

#[test]
fn entries_in_the_forbidden_zone_are_skipped_except_the_workspace() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    touch(root, "data/secret.txt");
    touch(root, "data/workspace/w.txt");
    touch(root, "keep.txt");
    // 工作区在数据根里面：照样进。
    let places = Places::here(root.join("data/workspace"), root.join("data"), None);
    let boundary = Boundary::new(&places);

    let index = Index::start(root.clone(), boundary, CAP, no_errors);
    let built = built(&index);
    let found = paths(&built);
    assert!(found.contains(&"keep.txt"));
    assert!(found.contains(&"data/workspace/"));
    assert!(found.contains(&"data/workspace/w.txt"));
    assert!(
        !found.contains(&"data/secret.txt"),
        "数据根里、工作区以外的不收：{found:?}"
    );
}

#[test]
fn a_build_still_finishes_when_results_are_read_mid_flight() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    for i in 0..300 {
        touch(root, &format!("f{i}.txt"));
    }
    let index = Index::start(root.clone(), permissive(root), CAP, no_errors);
    // 建到一半也能读：不要求等它建完。
    let _ = index.with(|built| built.entries.len());
    let built = built(&index);
    assert_eq!(built.entries.len(), 300);
    assert!(!built.partial);
}

#[cfg(unix)]
#[test]
fn unreadable_subdirectories_are_skipped_and_reported_through_the_callback() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if fs::metadata("/proc/self").expect("有").uid() == 0 {
        return;
    }
    let scratch = Scratch::new();
    let root = &scratch.0;
    touch(root, "locked/secret.txt");
    touch(root, "keep.txt");
    let locked = root.join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("改得了权限");

    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let into = Arc::clone(&seen);
    let index = Index::start(root.clone(), permissive(root), CAP, move |error| {
        into.lock().expect("锁没中毒").push(error);
    });
    let built = built(&index);

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("改得回来");
    assert!(paths(&built).contains(&"keep.txt"));
    assert!(
        !paths(&built).contains(&"locked/secret.txt"),
        "读不了的目录底下的不收：{:?}",
        paths(&built)
    );
    assert!(
        !seen.lock().expect("锁没中毒").is_empty(),
        "读不了一层目录，`on_error` 收到那一句"
    );
}
