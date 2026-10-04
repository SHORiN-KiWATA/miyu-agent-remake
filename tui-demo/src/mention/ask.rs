//! 问核心哪一个（蓝图 `tui.md`「`@` 文件列表」第 2 条）：带 `/`、`~` 打头、只打了 `@` 的问 `fs.list` 只读那一层，
//! 别的问 `fs.find` 模糊找；回应换成一条条（`protocol.md` 的 `fs.list`、`fs.find`）。

use std::path::PathBuf;

use serde_json::{Value, json};

use super::{Candidate, Status};

/// 问核心的一次。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// 按目录找：`dir` 是打的那一截目录（带最后的 `/`，`~` 打头的写成 `~/`），`prefix` 是名字的开头。
    List { dir: String, prefix: String },
    /// 模糊找；`fresh`：列表刚弹出来。
    Find { query: String, fresh: bool },
}

impl Ask {
    /// 照打的字定问哪一个。
    pub fn of(query: &str, fresh: bool) -> Ask {
        if !(query.is_empty() || query.contains('/') || query.starts_with('~')) {
            return Ask::Find {
                query: query.to_string(),
                fresh,
            };
        }
        let (dir, prefix) = match query.rfind('/') {
            Some(at) => (query[..=at].to_string(), &query[at + 1..]),
            // `~Doc`：家目录这一层里 `Doc` 开头的。
            None => match query.strip_prefix('~') {
                Some(rest) => ("~/".to_string(), rest),
                None => (String::new(), query),
            },
        };
        Ask::List {
            dir,
            prefix: prefix.to_string(),
        }
    }

    /// 方法名和参数；`cwd` 是界面所在的工作目录。
    pub fn request(&self, cwd: &str) -> (&'static str, Value) {
        match self {
            Ask::List { dir, prefix } => {
                ("fs.list", json!({"cwd": cwd, "dir": dir, "prefix": prefix}))
            }
            Ask::Find { query, fresh } => (
                "fs.find",
                json!({"cwd": cwd, "query": query, "fresh": fresh}),
            ),
        }
    }

    /// 回应换成一条条、怎么找来的、清单还在不在建。回了错的（`None`）当一条都没有。
    pub fn read(&self, result: Option<&Value>) -> (Vec<Candidate>, Status, bool) {
        let items = result
            .and_then(|r| r["items"].as_array())
            .map(Vec::as_slice)
            .unwrap_or_default();
        // 按目录找的照打的写法写：前面接上打的那一截目录，对上的字往后挪。
        let lead = match self {
            Ask::List { dir, .. } => dir.as_str(),
            Ask::Find { .. } => "",
        };
        let shift = lead.chars().count();
        let items = items
            .iter()
            .filter_map(|item| {
                Some(Candidate {
                    shown: format!("{lead}{}", item["path"].as_str()?),
                    path: PathBuf::from(item["full"].as_str()?),
                    dir: item["dir"].as_bool().unwrap_or(false),
                    hits: item["marks"]
                        .as_array()
                        .map(|m| {
                            m.iter()
                                .filter_map(Value::as_u64)
                                .map(|i| i as usize + shift)
                                .collect()
                        })
                        .unwrap_or_default(),
                })
            })
            .collect();
        let flag = |name: &str| result.is_some_and(|r| r[name].as_bool() == Some(true));
        let building = flag("building");
        let status = match self {
            Ask::List { .. } => Status::Layer,
            Ask::Find { .. } if building => Status::Indexing,
            Ask::Find { .. } if flag("partial") => Status::Partial,
            Ask::Find { .. } => Status::Full,
        };
        (items, status, building)
    }
}
