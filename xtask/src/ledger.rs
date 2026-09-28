//! 登记簿（`docs/designs/26-提示词.md` 第十节，J12）：给模型看的每一份字都登记在那张表里。
//!
//! 门禁查两边一一对上：`resources/` 下每一份文件都在表里，表里每一行都有文件，指纹对得上。指纹是
//! 文件字节的 SHA-256 的前 8 位：字改了就对不上，逼着回登记簿重新量 token、写上为什么改。算在
//! 「文档」那一项里。

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::drawing;

/// 登记簿在仓库里的位置。
pub const PATH: &str = "docs/designs/26-提示词.md";

pub(crate) const SECTION: &str = "登记簿";
pub(crate) const HEADER: [&str; 6] = [
    "文件",
    "进到哪",
    "什么时候加进来",
    "token",
    "为什么加",
    "指纹",
];

/// 给模型看的字都在这个目录里。
pub(crate) const RESOURCES: &str = "resources";

/// 给人看的字放在这样的目录里，不登记（施工 4-5 上）。
const HUMAN: &str = "human";

/// 查一遍，交回对不上的地方。
pub fn check(root: &Path) -> Vec<String> {
    let text = match std::fs::read_to_string(root.join(PATH)) {
        Ok(text) => text,
        Err(e) => return vec![format!("读不了登记簿 {PATH}：{e}")],
    };
    let rows = match rows(&text) {
        Ok(rows) => rows,
        Err(e) => return vec![format!("登记簿 {PATH} 读不出来：{e}")],
    };
    let mut files = BTreeMap::new();
    match walk(&root.join(RESOURCES), "", &mut files) {
        Ok(()) => compare(&rows, &files),
        Err(e) => vec![e],
    }
}

/// 表里的每一行：文件（`resources/` 下的路径）到登记的指纹。
fn rows(text: &str) -> Result<BTreeMap<String, String>, String> {
    let section = drawing::section(text, SECTION)?;
    let tables = drawing::tables(&section);
    let table = drawing::find_table(&tables, &HEADER)?;
    let mut rows = BTreeMap::new();
    for row in &table.rows {
        let quoted = |cell: Option<&String>| {
            cell.and_then(|cell| drawing::backticked(cell).into_iter().next())
                .ok_or_else(|| format!("这一行的文件、指纹要写在反引号里：{}", row.join(" | ")))
        };
        let file = quoted(row.first())?;
        let fingerprint = quoted(row.last())?;
        if rows.insert(file.clone(), fingerprint).is_some() {
            return Err(format!("{file} 登记了两行"));
        }
    }
    Ok(rows)
}

/// `dir` 下每一份文件：相对 `resources/` 的路径（用 `/` 分开，三个平台一样）到字节。
fn walk(dir: &Path, prefix: &str, files: &mut BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let unreadable = |e: std::io::Error| format!("读不了 {}：{e}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.file_type().map_err(unreadable)?.is_dir() {
            // 给人看的字不发给模型，不进登记簿（26 第十节，施工 4-5 上）。
            if name == HUMAN {
                continue;
            }
            walk(&entry.path(), &format!("{path}/"), files)?;
        } else {
            files.insert(path, std::fs::read(entry.path()).map_err(unreadable)?);
        }
    }
    Ok(())
}

/// 两边比：没登记的、登记了却没有文件的、指纹对不上的。
fn compare(rows: &BTreeMap<String, String>, files: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut problems = Vec::new();
    for (file, bytes) in files {
        let actual = fingerprint(bytes);
        match rows.get(file) {
            None => problems.push(format!(
                "resources/{file} 没登记：给模型看的字都要登记在 {PATH} 第十节（进到哪、什么时候加进来、多少 token、为什么加），指纹是 {actual}"
            )),
            Some(recorded) if *recorded != actual => problems.push(format!(
                "resources/{file} 的字变了，登记簿里的指纹还是 {recorded}：重新量 token、写上为什么改，指纹换成 {actual}"
            )),
            Some(_) => {}
        }
    }
    for file in rows.keys().filter(|file| !files.contains_key(*file)) {
        problems.push(format!("登记簿里有 {file}，resources/ 下却没有这份文件"));
    }
    problems
}

/// 指纹：字节的 SHA-256 的前 8 位十六进制。
fn fingerprint(bytes: &[u8]) -> String {
    Sha256::digest(bytes)[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
### 十、登记簿

| 文件 | 进到哪 | 什么时候加进来 | token | 为什么加 | 指纹 |
|---|---|---|---|---|---|
| `core/a.txt` | 事实 | 断了的那一次 | 3 | 施工 3-5 | `2cf24dba` |
| `core/b.txt` | 事实 | 每次 | 1 | 施工 3-5 | `00000000` |

### 附录
";

    fn files(entries: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        entries
            .iter()
            .map(|(path, text)| ((*path).to_string(), text.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn the_fingerprint_is_the_first_eight_hex_of_the_sha256() {
        assert_eq!(fingerprint(b"hello"), "2cf24dba");
    }

    #[test]
    fn the_rows_are_read_from_the_section_table() {
        let recorded = rows(TABLE).unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded["core/a.txt"], "2cf24dba");
        assert!(rows("### 十、别的\n").is_err());
        let twice = TABLE.replace("`core/b.txt`", "`core/a.txt`");
        assert!(rows(&twice).unwrap_err().contains("登记了两行"));
    }

    #[test]
    fn every_mismatch_is_named() {
        let recorded = rows(TABLE).unwrap();
        // a 对上了；b 的字变了；c 没登记。
        let problems = compare(
            &recorded,
            &files(&[
                ("core/a.txt", "hello"),
                ("core/b.txt", "x"),
                ("core/c.txt", "y"),
            ]),
        );
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("core/b.txt 的字变了"));
        assert!(problems[1].contains("core/c.txt 没登记"));
        // 登记了却没有文件。
        let problems = compare(&recorded, &files(&[("core/a.txt", "hello")]));
        assert_eq!(
            problems,
            ["登记簿里有 core/b.txt，resources/ 下却没有这份文件"]
        );
    }

    #[test]
    fn human_folders_are_not_walked() {
        let dir = std::env::temp_dir().join(format!("miyu-ledger-{}", std::process::id()));
        for (path, text) in [
            ("core/a.txt", "a"),
            ("core/human/zh.json", "{}"),
            ("software/x/human/en.json", "{}"),
            ("software/x/tools/t.json", "{}"),
        ] {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let mut found = BTreeMap::new();
        let walked = walk(&dir, "", &mut found);
        std::fs::remove_dir_all(&dir).unwrap();
        walked.unwrap();
        // 给人看的字不登记，别的照查。
        assert_eq!(
            found.keys().collect::<Vec<_>>(),
            ["core/a.txt", "software/x/tools/t.json"]
        );
    }
}
