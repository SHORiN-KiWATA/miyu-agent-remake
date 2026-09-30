//! 模糊找的清单（蓝图 `tui.md`「`@` 文件列表」第 2 条）：在后台线程里走一遍工作目录，认 `.gitignore`、跳过隐藏目录
//! 和配置里的几个目录；最多收 `cap` 个、最深 `depth` 层，收满就停。建到一半也能先拿去筛。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use ignore::WalkBuilder;

use crate::config::MentionLook;

/// 攒多少条交一次：少锁几次。
const BATCH: usize = 256;

/// 建好的、在建的清单。
#[derive(Debug, Default)]
pub struct Built {
    /// 每一条：相对工作目录的路径（`/` 隔开），是不是目录。
    pub entries: Vec<(String, bool)>,
    /// 走完了（收满了也算）。
    pub done: bool,
    /// 收满了，没走完：列的只是一部分。
    pub partial: bool,
}

/// 一份清单。
#[derive(Debug, Clone)]
pub struct Index {
    shared: Arc<Mutex<Built>>,
}

impl Index {
    /// 在后台建 `root` 的清单。
    pub fn start(root: PathBuf, look: &MentionLook) -> Self {
        let shared = Arc::new(Mutex::new(Built::default()));
        let into = shared.clone();
        let (cap, depth, skip) = (look.cap, look.depth, look.skip.clone());
        thread::spawn(move || walk(&root, cap, depth, &skip, &into));
        Self { shared }
    }

    /// 照现在建好的那部分做一件事。
    pub fn with<R>(&self, f: impl FnOnce(&Built) -> R) -> R {
        let built = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&built)
    }
}

fn walk(root: &PathBuf, cap: usize, depth: usize, skip: &[String], into: &Mutex<Built>) {
    let skip = skip.to_vec();
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .max_depth(Some(depth))
        .filter_entry(move |e| !skip.iter().any(|s| e.file_name() == s.as_str()))
        .build();
    let mut batch = Vec::with_capacity(BATCH);
    let mut count = 0;
    let mut partial = false;
    for entry in walker.flatten() {
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        if count >= cap {
            partial = true;
            break;
        }
        let path = relative.to_string_lossy().replace('\\', "/");
        let dir = entry.file_type().is_some_and(|t| t.is_dir());
        batch.push((path, dir));
        count += 1;
        if batch.len() >= BATCH {
            push(into, &mut batch);
        }
    }
    push(into, &mut batch);
    let mut built = into
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    built.done = true;
    built.partial = partial;
}

fn push(into: &Mutex<Built>, batch: &mut Vec<(String, bool)>) {
    let mut built = into
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    built.entries.append(batch);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::Index;
    use crate::config::Config;

    fn built(index: &Index) -> (Vec<String>, bool) {
        let until = Instant::now() + Duration::from_secs(5);
        while !index.with(|b| b.done) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        index.with(|b| {
            let mut names: Vec<String> = b.entries.iter().map(|(p, _)| p.clone()).collect();
            names.sort();
            (names, b.partial)
        })
    }

    #[test]
    fn hidden_ignored_and_skipped_dirs_stay_out_and_the_cap_stops_it() {
        let root = std::env::temp_dir().join(format!("miyu-index-{}", std::process::id()));
        for d in ["src", ".git", ".cache", "node_modules/pkg", "build"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        for f in [
            "src/main.rs",
            ".cache/x",
            "node_modules/pkg/a.js",
            "build/out.o",
            "keep.md",
        ] {
            std::fs::write(root.join(f), b"x").unwrap();
        }
        std::fs::write(root.join(".gitignore"), "build/\n").unwrap();
        let mut look = Config::builtin().unwrap().mention;
        let (names, partial) = built(&Index::start(root.clone(), &look));
        assert_eq!(
            names,
            ["keep.md", "src", "src/main.rs"],
            "隐藏的、.gitignore 的、skip 的都不收"
        );
        assert!(!partial);
        look.cap = 2;
        let (names, partial) = built(&Index::start(root.clone(), &look));
        assert_eq!(names.len(), 2);
        assert!(partial, "收满了就停，记着只是一部分");
        std::fs::remove_dir_all(&root).unwrap_or_default();
    }
}
