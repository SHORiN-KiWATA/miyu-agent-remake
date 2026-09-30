//! 找项目配置：往上找到仓库的根就停、到家目录就停、不看家目录本身、数据根里不找、只认最近的一份；给人看的路径。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::project::{config_in, find, shown};

/// 一个用完就删的临时目录（真实的位置），里面当家目录用。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-project-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Scratch(fs::canonicalize(dir).unwrap())
    }

    /// 建一个目录，交回它。
    fn dir(&self, path: &str) -> PathBuf {
        let dir = self.0.join(path);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 在 `repo` 里放一份项目配置。
    fn config(&self, repo: &str) -> PathBuf {
        let dir = self.dir(repo);
        fs::create_dir_all(dir.join(".miyu")).unwrap();
        fs::write(config_in(&dir), "").unwrap();
        dir
    }
}

impl Drop for Scratch {
    #[expect(clippy::let_underscore_must_use, reason = "删不掉就留在临时目录里")]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 数据根：放在家目录的 `.miyu`。
fn data_root(home: &Path) -> PathBuf {
    home.join(".miyu")
}

#[test]
fn the_nearest_one_upwards_is_found() {
    let home = Scratch::new();
    let repo = home.config("src/app");
    let deep = home.dir("src/app/crates/core/src");
    let found = find(&deep, Some(&home.0), &data_root(&home.0));
    assert_eq!(found, Some(repo.clone()));
    assert_eq!(
        find(&repo, Some(&home.0), &data_root(&home.0)),
        Some(repo.clone()),
        "就在这一层"
    );
    let inner = home.config("src/app/crates/core");
    assert_eq!(
        find(&deep, Some(&home.0), &data_root(&home.0)),
        Some(inner),
        "只认最近的一份"
    );
}

#[test]
fn the_search_stops_at_the_repository_root() {
    let home = Scratch::new();
    home.config("src");
    let repo = home.dir("src/app");
    fs::write(repo.join(".git"), "gitdir: elsewhere\n").unwrap();
    let deep = home.dir("src/app/lib");
    assert_eq!(
        find(&deep, Some(&home.0), &data_root(&home.0)),
        None,
        ".git 是文件也算"
    );
    fs::remove_file(repo.join(".git")).unwrap();
    fs::create_dir_all(repo.join(".git")).unwrap();
    assert_eq!(
        find(&deep, Some(&home.0), &data_root(&home.0)),
        None,
        ".git 是目录"
    );
}

#[test]
fn the_home_directory_itself_is_not_looked_at() {
    let home = Scratch::new();
    fs::create_dir_all(home.0.join(".miyu")).unwrap();
    fs::write(config_in(&home.0), "").unwrap();
    let work = home.dir("work");
    assert_eq!(find(&work, Some(&home.0), Path::new("/nowhere")), None);
    assert_eq!(find(&home.0, Some(&home.0), Path::new("/nowhere")), None);
}

#[test]
fn nothing_inside_the_data_root_is_searched() {
    let home = Scratch::new();
    let root = home.config("data");
    let inside = home.dir("data/home/admin/workspace");
    assert_eq!(find(&inside, Some(&home.0), &root), None);
}

#[test]
fn outside_the_home_it_stops_at_the_root() {
    let home = Scratch::new();
    let other = Scratch::new();
    let deep = other.dir("a/b");
    assert_eq!(find(&deep, Some(&home.0), &data_root(&home.0)), None);
    let repo = other.config("a");
    assert_eq!(find(&deep, Some(&home.0), &data_root(&home.0)), Some(repo));
}

#[test]
fn paths_under_home_are_shown_with_a_tilde() {
    let home = Scratch::new();
    let repo = home.config("src/app");
    assert_eq!(
        shown(&config_in(&repo), Some(&home.0)),
        "~/src/app/.miyu/config.toml"
    );
    assert_eq!(shown(&home.0, Some(&home.0)), "~");
    let other = Scratch::new();
    assert_eq!(
        shown(&other.0, Some(&home.0)),
        other.0.to_string_lossy().trim_start_matches(r"\\?\"),
        "家目录外面的照原样"
    );
}
