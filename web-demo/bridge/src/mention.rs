//! `web.files`：`@` 选文件（蓝图 `web.md`「`@` 选文件」，照设计 13 H14、`tui.md`「`@` 文件列表」）。页面读不到本机的目录，
//! 桥替它列、替它找；核心以后经协议给，形状照这一份。
//!
//! - 按目录找（`mode: dir`）：像 shell 补全那样只读那一层（`~` 是家目录，相对路径照会话的工作目录），名字照开头对、不论
//!   大小写；点开头的打了 `.` 才列；目录在前、文件在后，各照名字排。
//! - 模糊找（`mode: find`）：在工作目录里建一份清单（`ignore` 库，和核心的 `glob`、`grep` 同一套：认 `.gitignore`、跳过隐藏
//!   目录和 `skip`），最多 `cap` 个、最深 `depth` 层，收满就停（`partial`）；打的字照先后都在路径里的才列，照 `score` 排，
//!   最多 `shown` 条。页面开列表时带 `fresh`：清单隔 `refresh_secs` 以上的重建。
//! - Miyu 自己的数据目录不列、不找（和 `/file` 同一条）。
//!
//! 数都在页面目录下的 `resources/mention.json`。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// `resources/mention.json`。
pub struct Config {
    cap: usize,
    depth: usize,
    skip: Vec<String>,
    shown: usize,
    refresh: Duration,
}

impl Config {
    /// 读页面目录下的 `resources/mention.json`。
    ///
    /// # Errors
    ///
    /// 读不到、不是 JSON、少了哪一格：说是哪一样。
    pub fn load(dir: &Path) -> Result<Config, String> {
        let path = dir.join("resources/mention.json");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("读不了 {}：{e}", path.display()))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{} 不是 JSON：{e}", path.display()))?;
        let num = |k: &str| v[k].as_u64().ok_or_else(|| format!("{} 少了 {k}", path.display()));
        let skip = v["skip"].as_array().ok_or_else(|| format!("{} 少了 skip", path.display()))?;
        Ok(Config {
            cap: usize::try_from(num("cap")?).unwrap_or(usize::MAX),
            depth: usize::try_from(num("depth")?).unwrap_or(usize::MAX),
            skip: skip.iter().filter_map(|s| s.as_str().map(str::to_string)).collect(),
            shown: usize::try_from(num("shown")?).unwrap_or(usize::MAX),
            refresh: Duration::from_secs(num("refresh_secs")?),
        })
    }
}

/// 模糊找的一份清单：哪个工作目录的、什么时候建的、里面的路径（相对工作目录，`/` 分隔）、收满了没有。
struct Index {
    cwd: PathBuf,
    built: Instant,
    entries: Vec<(String, bool)>,
    partial: bool,
}

/// 列、找文件；模糊找的清单记着一份。
pub struct Mention {
    config: Config,
    index: Mutex<Option<Index>>,
}

impl Mention {
    /// 读配置。
    ///
    /// # Errors
    ///
    /// 同 `Config::load`。
    pub fn load(dir: &Path) -> Result<Mention, String> {
        Ok(Mention { config: Config::load(dir)?, index: Mutex::new(None) })
    }

    /// 一次 `web.files`：`{cwd, mode: "dir", dir, prefix}` 或 `{cwd, mode: "find", query, fresh}`。交回
    /// `{items: [{path, full, dir, size?, type?, marks}], partial, layer}`：`path` 是列表上写的，`full` 是绝对路径，`type` 是
    /// 照扩展名认的媒体类型（附件照它认收不收），`marks` 是 `path` 里对上的字（第几个字）。`data` 是 Miyu 的数据目录（不列）；
    /// `kind` 照扩展名认媒体类型（桥给本机文件的那一张表，`media.json`）。
    ///
    /// # Errors
    ///
    /// 工作目录、要读的目录不在、读不了，在数据目录里面：说是哪一样。
    pub fn request(&self, params: &Value, data: &Path, kind: &dyn Fn(&Path) -> String) -> Result<Value, String> {
        let cwd = Path::new(params["cwd"].as_str().unwrap_or(""));
        if !cwd.is_absolute() || !cwd.is_dir() {
            return Err(format!("工作目录不在：{}", cwd.display()));
        }
        let data = data.canonicalize().unwrap_or_else(|_| data.to_path_buf());
        match params["mode"].as_str() {
            Some("dir") => self.list(cwd, params["dir"].as_str().unwrap_or(""), params["prefix"].as_str().unwrap_or(""), &data, kind),
            Some("find") => self.find(cwd, params["query"].as_str().unwrap_or(""), params["fresh"].as_bool().unwrap_or(false), &data, kind),
            other => Err(format!("不认识的找法：{other:?}")),
        }
    }

    /// 按目录找：`dir` 是打的那一截目录（`~` 打头的照家目录，绝对的照原样，别的照工作目录），`prefix` 是后面名字的开头。
    fn list(&self, cwd: &Path, dir: &str, prefix: &str, data: &Path, kind: &dyn Fn(&Path) -> String) -> Result<Value, String> {
        let base = if dir == "~" || dir.starts_with("~/") {
            std::env::home_dir().ok_or("不知道家目录在哪")?.join(dir.trim_start_matches('~').trim_start_matches('/'))
        } else {
            cwd.join(dir)
        };
        let real = base.canonicalize().map_err(|e| format!("读不了 {}：{e}", base.display()))?;
        if real.starts_with(data) {
            return Err("这是 Miyu 自己的数据，不列".to_string());
        }
        let want = prefix.to_lowercase();
        let entries = std::fs::read_dir(&real).map_err(|e| format!("读不了 {}：{e}", real.display()))?;
        let mut items: Vec<(bool, String, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let hidden = name.starts_with('.') && !prefix.starts_with('.');
                let full = e.path();
                let inside = full.canonicalize().map(|r| r.starts_with(data)).unwrap_or(false);
                (!hidden && !inside && name.to_lowercase().starts_with(&want)).then(|| (full.is_dir(), name, full))
            })
            .collect();
        items.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));
        let partial = items.len() > self.config.shown;
        items.truncate(self.config.shown);
        let items: Vec<Value> = items.into_iter().map(|(is_dir, name, full)| {
            let shown = if is_dir { format!("{name}/") } else { name };
            let marks: Vec<usize> = (0..prefix.chars().count()).collect();
            item(&shown, &full, is_dir, marks, kind)
        }).collect();
        Ok(json!({"items": items, "partial": partial, "layer": true}))
    }

    /// 模糊找：清单没有、换了工作目录、`fresh` 且隔了 `refresh` 以上的，重建一份；照 `score` 排，最多 `shown` 条。
    fn find(&self, cwd: &Path, query: &str, fresh: bool, data: &Path, kind: &dyn Fn(&Path) -> String) -> Result<Value, String> {
        let mut index = self.index.lock().map_err(|_| "清单坏了".to_string())?;
        let stale = match index.as_ref() {
            Some(i) => i.cwd != cwd || (fresh && i.built.elapsed() >= self.config.refresh),
            None => true,
        };
        if stale {
            *index = Some(self.build(cwd, data));
        }
        let Some(i) = index.as_ref() else { return Err("清单坏了".to_string()) };
        let mut hits: Vec<(i64, usize, &str, bool, Vec<usize>)> = i.entries.iter().filter_map(|(path, is_dir)| {
            let shown = path.as_str();
            score(shown, query).map(|(s, marks)| (s, shown.chars().count(), shown, *is_dir, marks))
        }).collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then_with(|| a.2.cmp(b.2)));
        hits.truncate(self.config.shown);
        let items: Vec<Value> = hits.into_iter().map(|(_, _, path, is_dir, marks)| {
            let full = cwd.join(path.trim_end_matches('/'));
            item(path, &full, is_dir, marks, kind)
        }).collect();
        Ok(json!({"items": items, "partial": i.partial, "layer": false}))
    }

    /// 建清单：认 `.gitignore`、跳过隐藏目录和 `skip`、数据目录，最深 `depth` 层，收满 `cap` 个就停。目录后面带 `/`。
    fn build(&self, cwd: &Path, data: &Path) -> Index {
        let skip = self.config.skip.clone();
        let data = data.to_path_buf();
        let walker = ignore::WalkBuilder::new(cwd)
            .max_depth(Some(self.config.depth))
            .require_git(false)
            .filter_entry(move |e| {
                let name = e.file_name().to_string_lossy();
                !skip.iter().any(|s| *s == name) && !e.path().starts_with(&data)
            })
            .build();
        let mut entries = Vec::new();
        let mut partial = false;
        for entry in walker.filter_map(Result::ok) {
            if entry.depth() == 0 {
                continue;
            }
            if entries.len() >= self.config.cap {
                partial = true;
                break;
            }
            let Ok(rel) = entry.path().strip_prefix(cwd) else { continue };
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            let mut path = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            if is_dir {
                path.push('/');
            }
            entries.push((path, is_dir));
        }
        Index { cwd: cwd.to_path_buf(), built: Instant::now(), entries, partial }
    }
}

/// 列表上的一条：写的字、绝对路径、是不是目录、文件的大小（附件照它挡太大的）、媒体类型（附件照它认收不收）、对上的字。
fn item(shown: &str, full: &Path, is_dir: bool, marks: Vec<usize>, kind: &dyn Fn(&Path) -> String) -> Value {
    let size = if is_dir { None } else { std::fs::metadata(full).ok().map(|m| m.len()) };
    let media = (!is_dir).then(|| kind(full));
    json!({"path": shown, "full": full.display().to_string(), "dir": is_dir, "size": size, "type": media, "marks": marks})
}

/// 一段的开头：路径的头一个字，或者前面是 `/`、`-`、`_`、`.`、空格。
fn starts_part(chars: &[char], at: usize) -> bool {
    at == 0 || matches!(chars[at - 1], '/' | '-' | '_' | '.' | ' ')
}

/// 模糊找的分（照 `tui.md`「`@` 文件列表」第 3 条）：打的字照先后都在路径里（不论大小写）才有分，交回分和对上的字在路径里
/// 是第几个字；对不上的交 `None`。先试整个落在文件名里，落不下的再从路径开头找；每个字对上 1 分，落在文件名里的多 3 分，
/// 在一段开头的多 8 分，和上一个字连着的多 5 分；文件名去掉扩展名正好是打的字的多 100 分。空的都对得上、0 分。
pub fn score(path: &str, query: &str) -> Option<(i64, Vec<usize>)> {
    let chars: Vec<char> = path.to_lowercase().chars().collect();
    let want: Vec<char> = query.to_lowercase().chars().collect();
    if want.is_empty() {
        return Some((0, Vec::new()));
    }
    let trimmed = path.trim_end_matches('/');
    let name_at = trimmed.rfind('/').map_or(0, |i| trimmed[..=i].chars().count());
    let take = |from: usize| -> Option<Vec<usize>> {
        let mut marks = Vec::with_capacity(want.len());
        let mut at = from;
        for w in &want {
            // 往后找这个字：一段开头的那一处优先（在下一个一段开头之前找得到的话）
            let first = (at..chars.len()).find(|&i| chars[i] == *w)?;
            let better = (first..chars.len()).take_while(|&i| i == first || !starts_part(&chars, i) || chars[i] == *w)
                .find(|&i| chars[i] == *w && starts_part(&chars, i));
            let hit = if marks.last().is_some_and(|&l: &usize| l + 1 == first) { first } else { better.unwrap_or(first) };
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
    let name: String = chars[name_at..].iter().collect::<String>();
    let stem = name.trim_end_matches('/').split('.').next().unwrap_or("");
    if stem == want.iter().collect::<String>() {
        total += 100;
    }
    Some((total, marks))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一个用完就删的目录。
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(files: &[&str]) -> Scratch {
            let mut bytes = [0u8; 8];
            getrandom::fill(&mut bytes).expect("随机数");
            let name: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            let root = std::env::temp_dir().join(format!("miyu-mention-{name}"));
            for f in files {
                let p = root.join(f);
                if f.ends_with('/') {
                    std::fs::create_dir_all(&p).expect("建目录");
                } else {
                    std::fs::create_dir_all(p.parent().expect("上一层")).expect("建目录");
                    std::fs::write(&p, "x").expect("写文件");
                }
            }
            Scratch(root)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            drop(std::fs::remove_dir_all(&self.0));
        }
    }

    fn mention(cap: usize, depth: usize) -> Mention {
        let config = Config { cap, depth, skip: vec!["node_modules".into(), "target".into()], shown: 50, refresh: Duration::from_secs(10) };
        Mention { config, index: Mutex::new(None) }
    }

    /// 测试里认媒体类型：一律当文字
    fn plain(_: &Path) -> String {
        "text/plain".to_string()
    }

    fn paths(v: &Value) -> Vec<String> {
        v["items"].as_array().expect("items").iter().map(|x| x["path"].as_str().unwrap_or("").to_string()).collect()
    }

    #[test]
    fn 模糊找的分_打的字照先后都在才有_文件名里的_一段开头的_连着的_文件名正好是的分高() {
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
    }

    #[test]
    fn 按目录找_名字照开头对_点开头的打了点才列_目录在前() {
        let dir = Scratch::new(&["b.txt", "a/", "Abc.md", ".hidden", "zeta/", "ab.rs"]);
        let m = mention(100, 8);
        let data = Path::new("/nonexistent-miyu-data");
        let cwd = dir.0.display().to_string();
        let all = m.request(&json!({"cwd": cwd, "mode": "dir", "dir": "", "prefix": ""}), data, &plain).expect("列");
        assert_eq!(paths(&all), vec!["a/", "zeta/", "ab.rs", "Abc.md", "b.txt"]);
        assert_eq!(all["layer"], json!(true));
        let a = m.request(&json!({"cwd": cwd, "mode": "dir", "dir": "", "prefix": "a"}), data, &plain).expect("列");
        assert_eq!(paths(&a), vec!["a/", "ab.rs", "Abc.md"]);
        let dot = m.request(&json!({"cwd": cwd, "mode": "dir", "dir": "", "prefix": "."}), data, &plain).expect("列");
        assert_eq!(paths(&dot), vec![".hidden"]);
        let full = all["items"][2]["full"].as_str().expect("full");
        assert!(full.ends_with("ab.rs") && Path::new(full).is_absolute());
        assert_eq!(all["items"][2]["size"], json!(1));
    }

    #[test]
    fn 模糊找_认gitignore_跳过隐藏目录和skip_最深几层_收满就停() {
        let dir = Scratch::new(&[".gitignore", "src/main.rs", "src/domain/x.rs", "out/main.log", ".git/main", "node_modules/main.js", "a/b/c/main.txt"]);
        std::fs::write(dir.0.join(".gitignore"), "out/\n").expect("写");
        let data = Path::new("/nonexistent-miyu-data");
        let cwd = dir.0.display().to_string();
        let found = mention(100, 8).request(&json!({"cwd": cwd, "mode": "find", "query": "main", "fresh": true}), data, &plain).expect("找");
        let got = paths(&found);
        assert_eq!(got.first().map(String::as_str), Some("src/main.rs"), "文件名正好是的排前面：{got:?}");
        assert!(got.contains(&"a/b/c/main.txt".to_string()));
        assert!(!got.iter().any(|p| p.starts_with("out/") || p.starts_with(".git") || p.starts_with("node_modules")), "{got:?}");
        assert_eq!(found["partial"], json!(false));
        let shallow = mention(100, 2).request(&json!({"cwd": cwd, "mode": "find", "query": "main", "fresh": true}), data, &plain).expect("找");
        assert!(!paths(&shallow).contains(&"a/b/c/main.txt".to_string()), "最深两层");
        let capped = mention(2, 8).request(&json!({"cwd": cwd, "mode": "find", "query": "", "fresh": true}), data, &plain).expect("找");
        assert_eq!(capped["partial"], json!(true));
    }

    #[test]
    fn 数据目录不列() {
        let dir = Scratch::new(&["data/secret.txt"]);
        let data = dir.0.join("data");
        let cwd = dir.0.display().to_string();
        assert!(mention(100, 8).request(&json!({"cwd": cwd, "mode": "dir", "dir": "data", "prefix": ""}), &data, &plain).is_err());
        let all = mention(100, 8).request(&json!({"cwd": cwd, "mode": "dir", "dir": "", "prefix": ""}), &data, &plain).expect("列");
        assert!(paths(&all).is_empty(), "数据目录本身也不列");
    }
}
