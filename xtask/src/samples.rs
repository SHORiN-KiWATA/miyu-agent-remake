//! 蓝图里标着「样本」的块（`docs/blueprint/README.md` 第三节，施工 4-9 三补）：块前一行以 `` 样本 `<路径>` `` 开头，
//! 块里的字加一个换行，就是那份文件的全部字节。门禁照着逐字节比，只改一边，另一边就红。算在「文档」那一项里。

use std::path::Path;

use crate::drawing;

/// 蓝图在仓库里的位置。
const BLUEPRINT: &str = "docs/blueprint";

/// 样本块前面那一行的开头。
const MARK: &str = "样本 ";

/// 查蓝图的每一页，交回对不上的地方。
pub fn check(root: &Path) -> Vec<String> {
    let mut pages = Vec::new();
    if let Err(e) = markdown(&root.join(BLUEPRINT), BLUEPRINT, &mut pages) {
        return vec![e];
    }
    pages.sort();
    let mut problems = Vec::new();
    for (name, path) in pages {
        match std::fs::read_to_string(&path) {
            Ok(text) => problems.extend(page(&name, &text, |file| {
                std::fs::read(root.join(file)).map_err(|e| e.to_string())
            })),
            Err(e) => problems.push(format!("读不了 {name}：{e}")),
        }
    }
    problems
}

/// `dir` 下的每一页 Markdown：仓库里的写法（用 `/` 分开）和路径。
fn markdown(
    dir: &Path,
    prefix: &str,
    pages: &mut Vec<(String, std::path::PathBuf)>,
) -> Result<(), String> {
    let unreadable = |e: std::io::Error| format!("读不了 {}：{e}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        if entry.file_type().map_err(unreadable)?.is_dir() {
            markdown(&entry.path(), &name, pages)?;
        } else if name.ends_with(".md") {
            pages.push((name, entry.path()));
        }
    }
    Ok(())
}

/// 查一页 `name`：每一个样本块和它指到的文件比，文件照 `read` 读。
pub(crate) fn page(
    name: &str,
    text: &str,
    read: impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Vec<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut problems = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let line = lines[at];
        at += 1;
        if !line.starts_with(MARK) {
            continue;
        }
        let number = at;
        let Some(file) = drawing::backticked(line).into_iter().next() else {
            problems.push(format!(
                "{name} 第 {number} 行标着样本，没写指到哪份文件：{line}"
            ));
            continue;
        };
        // 下一个不空的行是块的开头。
        while at < lines.len() && lines[at].trim().is_empty() {
            at += 1;
        }
        let Some(fence) = lines.get(at).and_then(|line| opening(line)) else {
            problems.push(format!("{name} 第 {number} 行标着样本，后面没有块"));
            continue;
        };
        at += 1;
        let start = at;
        while at < lines.len() && lines[at].trim_end() != fence {
            at += 1;
        }
        if at == lines.len() {
            problems.push(format!("{name} 第 {number} 行的样本块没有收尾"));
            break;
        }
        let block = format!("{}\n", lines[start..at].join("\n"));
        at += 1;
        match read(&file) {
            Err(e) => problems.push(format!("{name} 第 {number} 行的样本指到 {file}，读不了：{e}")),
            Ok(bytes) if bytes == block.as_bytes() => {}
            Ok(bytes) => problems.push(format!(
                "{name} 第 {number} 行的样本和 {file} 对不上，从第 {} 个字节起不一样：改了一边，另一边照着改",
                first_difference(block.as_bytes(), &bytes) + 1
            )),
        }
    }
    problems
}

/// 块的开头：三个以上的反引号，后面可以跟语言。交回收尾要写的那一串反引号。
fn opening(line: &str) -> Option<&str> {
    let ticks = line.len() - line.trim_start_matches('`').len();
    (ticks >= 3).then(|| &line[..ticks])
}

/// 两串字节从哪里起不一样（从 0 数）；一串是另一串的开头的，是短的那串的长度。
fn first_difference(a: &[u8], b: &[u8]) -> usize {
    a.iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()))
}

#[cfg(test)]
mod tests;
