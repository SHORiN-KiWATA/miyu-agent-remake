//! 模糊找文件（`fs.find`，施工 W-2，`docs/blueprint/web-module.md`「三、列文件、找文件」第 4、6、7、9 条）：在一个
//! 目录里建一份清单，后台建、随时能读建到现在的那部分；打分、排序。
//!
//! 清单在后台线程里建（不是请求的那个线程），不挡别的请求；头隔一阵再问，读到的是建到现在的那部分，`done` 说
//! 建完了没有。清单记几份、`fresh` 时多久重建是协议端点的事（`crates/miyu-endpoint/src/files.rs`），这里只管
//! 怎么走一遍目录、怎么打分。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use ignore::WalkBuilder;

use crate::boundary::{Boundary, Zone};

/// 一次列（`fs.list`、`fs.find` 共用）最多几条，多了截掉（`web-module.md`「怎么走」第 3、7 条）。
pub const SHOWN: usize = 50;
/// 清单最多收几个文件，收满就停（出厂值，照 `jobs.output_chars` 的放法，配置那一步能改）。
pub const CAP: usize = 20_000;
/// 最深走几层。
pub const DEPTH: usize = 8;
/// `fresh` 时，清单建好多少秒以上才重建。
pub const FRESH_SECS: u64 = 10;
/// 核心最多记几个目录的清单，多了丢最久没用的。
pub const MAX_INDEXES: usize = 4;
/// 出厂跳过的名单，连着隐藏目录一起不进。
const SKIP: [&str; 2] = ["node_modules", "target"];
/// 攒多少条交一次：少锁几次。
const BATCH: usize = 256;

/// 清单里的一条：相对清单的根、用 `/` 连起来的路径（目录带 `/`），是不是目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// 相对清单的根、用 `/` 连起来的路径：目录带 `/`。
    pub path: String,
    /// 是不是目录。
    pub dir: bool,
}

/// 建到现在的清单。
#[derive(Debug, Default)]
pub struct Built {
    /// 收到的，先后不定（后台线程边走边收）。
    pub entries: Vec<Found>,
    /// 走完了（收满了也算）。
    pub done: bool,
    /// 收满了、没走完：清单只是一部分。
    pub partial: bool,
}

/// 一份清单：在后台线程里建，随时能照 [`Index::with`] 读建到现在的那部分。
#[derive(Clone)]
pub struct Index {
    shared: Arc<Mutex<Built>>,
}

impl Index {
    /// 在后台线程里建 `root` 的清单：认 `.gitignore`（不要求是 git 仓库），跳过隐藏的、出厂名单、落进边界表
    /// 「谁都不能碰」那一片的（`boundary`，工作区除外），不跟链接；最深 [`DEPTH`] 层，收满 `cap` 个就停（出厂用
    /// [`CAP`]；测试传小一点的，不用真造两万个文件）。读不了一层目录的（没有权限这类），`on_error` 收到那一句
    /// 原话，跳过它接着建。
    pub fn start(
        root: PathBuf,
        boundary: Boundary,
        cap: usize,
        on_error: impl FnMut(String) + Send + 'static,
    ) -> Index {
        let shared = Arc::new(Mutex::new(Built::default()));
        let into = Arc::clone(&shared);
        thread::spawn(move || walk(&root, &boundary, cap, &into, on_error));
        Index { shared }
    }

    /// 照现在建到的那部分做一件事：拿着锁的这一刻看到的快照。
    pub fn with<R>(&self, f: impl FnOnce(&Built) -> R) -> R {
        let built = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&built)
    }
}

/// 走一遍 `root`，把收到的分批写进 `into`；完了把 `done`、`partial` 置好。
fn walk(
    root: &Path,
    boundary: &Boundary,
    cap: usize,
    into: &Mutex<Built>,
    mut on_error: impl FnMut(String),
) {
    let for_descent = boundary.clone();
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .follow_links(false)
        .max_depth(Some(DEPTH))
        .require_git(false)
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            if SKIP.iter().any(|skip| *skip == name) {
                return false;
            }
            let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
            // 工作区常常就在数据根里面：挡住的是进不了工作区的那几片，不是数据根这一层本身（穿过去才找得到）。
            !is_dir || !for_descent.blocks_descent(entry.path())
        })
        .build();
    let mut batch = Vec::with_capacity(BATCH);
    let mut count = 0;
    let mut partial = false;
    for result in walker {
        match result {
            Err(error) => on_error(error.to_string()),
            Ok(entry) if entry.depth() == 0 => {}
            // 落进「谁都不能碰」那一片的不收：可能只是借道去找底下的工作区（施工 W-2）。
            Ok(entry) if boundary.zone(entry.path()) == Zone::Forbidden => {}
            Ok(entry) => {
                if count >= cap {
                    partial = true;
                    break;
                }
                let Ok(relative) = entry.path().strip_prefix(root) else {
                    continue;
                };
                let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
                let mut path = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                if is_dir {
                    path.push('/');
                }
                batch.push(Found { path, dir: is_dir });
                count += 1;
                if batch.len() >= BATCH {
                    push(into, &mut batch);
                }
            }
        }
    }
    push(into, &mut batch);
    let mut built = into
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    built.done = true;
    built.partial = partial;
}

fn push(into: &Mutex<Built>, batch: &mut Vec<Found>) {
    if batch.is_empty() {
        return;
    }
    let mut built = into
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    built.entries.append(batch);
}

/// 一段的开头：路径的头一个字，或者前面是 `/`、`-`、`_`、`.`、空格。
const SEPARATORS: [char; 5] = ['/', '-', '_', '.', ' '];

/// 打分（`web-module.md`「怎么走」第 7 条，照 proto/web-demo 分支 `web-demo/bridge/src/mention.rs` 的
/// `score`）：`query` 的字照先后都在 `path` 里（不论大小写）才算，交回分和对上的是第几个字（按字符数）；对不上
/// 的是 `None`。先试整个落在文件名里，落不下再从路径开头找；每个字对上 1 分，落在文件名里多 3 分，在一段的
/// 开头多 8 分，和上一个字连着多 5 分；文件名去掉扩展名正好是打的字多 100 分。`query` 是空的都对得上、0 分。
#[must_use]
pub fn score(path: &str, query: &str) -> Option<(i64, Vec<usize>)> {
    let chars: Vec<char> = path.to_lowercase().chars().collect();
    let want: Vec<char> = query.to_lowercase().chars().collect();
    if want.is_empty() {
        return Some((0, Vec::new()));
    }
    let trimmed = path.trim_end_matches('/');
    let name_at = trimmed
        .rfind('/')
        .map_or(0, |at| trimmed[..=at].chars().count());
    let take = |from: usize| -> Option<Vec<usize>> {
        let mut marks = Vec::with_capacity(want.len());
        let mut at = from;
        for w in &want {
            let first = (at..chars.len()).find(|&i| chars[i] == *w)?;
            let better = (first..chars.len())
                .take_while(|&i| i == first || !starts_part(&chars, i) || chars[i] == *w)
                .find(|&i| chars[i] == *w && starts_part(&chars, i));
            let hit = if marks.last().is_some_and(|&last: &usize| last + 1 == first) {
                first
            } else {
                better.unwrap_or(first)
            };
            marks.push(hit);
            at = hit + 1;
        }
        Some(marks)
    };
    let marks = take(name_at).or_else(|| take(0))?;
    let mut total = 0i64;
    for (k, &i) in marks.iter().enumerate() {
        total += 1;
        if i >= name_at {
            total += 3;
        }
        if starts_part(&chars, i) {
            total += 8;
        }
        if k > 0 && marks[k - 1] + 1 == i {
            total += 5;
        }
    }
    let name: String = chars[name_at..].iter().collect();
    let stem = name.trim_end_matches('/').split('.').next().unwrap_or("");
    if stem == want.iter().collect::<String>() {
        total += 100;
    }
    Some((total, marks))
}

/// `at` 是不是一段的开头：路径的头一个字，或者前面是 [`SEPARATORS`] 里的一个。
fn starts_part(chars: &[char], at: usize) -> bool {
    at == 0 || SEPARATORS.contains(&chars[at - 1])
}

#[cfg(test)]
mod tests;
