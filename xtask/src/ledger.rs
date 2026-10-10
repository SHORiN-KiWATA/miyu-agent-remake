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

/// 资源目录最上一层的这个目录放模型资料，是数据，不发给模型，不登记（施工 6-3 上）。
const DATA: &str = "models";

/// 资源目录最上一层的这个目录放网页软件的设置（`web.json`）和页面，给浏览器的，不发给模型，不登记（施工 W-9，`web-ui.md`）。
const WEB: &str = "web";

/// 资源目录最上一层的这个目录放软件包清单（施工 9-1 上，`packages.md`）：给人看的名字、说明和程序名，不发给模型，不登记。
const PACKAGES: &str = "packages";

/// 资源目录最上一层的这个目录放出厂的预设（施工 P-2 上，`presets.md`）：给人看的名字、说明和开关表，不发给模型，不登记。
const PRESETS: &str = "presets";

/// `software/mermaid/`、`software/net/` 整个是数据（字体、三种记号色、源码的上限；抓链接卡片的时限、上限、请求头），
/// 不发给模型，不登记（施工 W-4，`mermaid.md`「样子」：「这条线不加给模型看的字」；施工 W-7，`net.md`）。
const SOFTWARE_DIR: &str = "software/";

/// 人格目录里的 `persona.toml` 是给人看的名字、说明和几项配置，不发给模型，不登记（施工 P-1 下，`personas.md`）；同一个目录里
/// `prompts/` 下的照查。
const PERSONAS_DIR: &str = "personas/";
const PERSONA_TOML: &str = "persona.toml";

/// `software/onebot/` 里这几份是数据，不发给模型，不登记：群聊内核的出厂参数（几张表的数，施工 O-15，`chat.md` 第八条施工时定的
/// 第 16 条）、违规词表（只拿来比子串）、出厂的场所规则（施工 O-15 下，第八条施工时定的第 21 条），桥自己的数（路径、等回应多久、
/// 队列多长，施工 O-8，`onebot.md`「施工时定的」第 15 条）。只豁免这几份、不豁免整个包（包里有给模型看的字，判官）：别处同名的、
/// `venues.d/` 里别的文件照查。
const ONEBOT_DATA: [&str; 4] = [
    "software/onebot/bridge.json",
    "software/onebot/defaults.toml",
    "software/onebot/moderation.txt",
    "software/onebot/venues.d/50-defaults.toml",
];
const DATA_PACKAGES: [&str; 2] = ["mermaid", "net"];

/// `core/view.json` 是数据，不发给模型，不登记：视图投影照它认哪件工具算命令、编辑、子代理、留言（施工 9-8 上，`view.md`）。
/// `core/memory/secrets.toml` 也是：抽取照它认常见的 key 写法、遮掉，不发给模型（施工 R-6 上，`memory.md` 第六条）。只豁免
/// 这几份：`core/` 里别的照查。
const CORE_DATA: [&str; 2] = ["core/view.json", "core/memory/secrets.toml"];

/// 这几个单独的目录整个是给浏览器的，不发给模型，不登记：`software/onebot/web/` 是 QQ 桥 WebUI 的页面（施工 O-16，
/// `onebot.md` 第二条；照最上一层 `web/` 的先例）。只豁免这一个目录，不豁免整个包。
const DATA_DIRS: [&str; 1] = ["software/onebot/web"];

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
            // 给人看的字、模型资料、mermaid 的 style.json、net 的 link_preview.json、网页软件的、QQ 桥 WebUI 的页面都不发给
            // 模型，不进登记簿（26 第十节，施工 4-5 上、6-3 上、W-4、W-7、W-9、O-16）。
            if name == HUMAN
                || (prefix.is_empty()
                    && (name == DATA || name == WEB || name == PACKAGES || name == PRESETS))
                || (prefix == SOFTWARE_DIR && DATA_PACKAGES.contains(&name.as_str()))
                || DATA_DIRS.contains(&path.as_str())
            {
                continue;
            }
            walk(&entry.path(), &format!("{path}/"), files)?;
        } else if !(name == PERSONA_TOML
            && prefix.starts_with(PERSONAS_DIR)
            && prefix.matches('/').count() == 2)
            && !ONEBOT_DATA.contains(&path.as_str())
            && !CORE_DATA.contains(&path.as_str())
        {
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
            ("core/view.json", "{}"),
            ("core/x/view.json", "{}"),
            ("core/human/zh.json", "{}"),
            ("software/x/human/en.json", "{}"),
            ("software/x/tools/t.json", "{}"),
            ("models/models-dev.json", "{}"),
            ("web/web.json", "{}"),
            ("web/pages/index.html", "x"),
            ("core/web/w.txt", "w"),
            ("core/models/m.txt", "m"),
            ("software/mermaid/style.json", "{}"),
            ("software/net/link_preview.json", "{}"),
            ("software/x/mermaid/not_special_here.json", "{}"),
            ("software/x/net/not_special_here.json", "{}"),
            ("personas/x/persona.toml", "[persona]"),
            ("personas/x/prompts/persona.md", "p"),
            ("personas/x/prompts/persona.toml", "q"),
            ("software/onebot/defaults.toml", "[chatty]"),
            ("software/onebot/judge/system.txt", "s"),
            ("software/x/defaults.toml", "d"),
            ("software/onebot/moderation.txt", "spam"),
            ("software/onebot/venues.d/50-defaults.toml", "[[rule]]"),
            ("software/onebot/venues.d/60-more.toml", "[[rule]]"),
            ("software/x/moderation.txt", "m"),
            ("packages/web.toml", "[package]"),
            ("core/packages/p.txt", "p"),
            ("presets/dev.toml", "[preset]"),
            ("core/presets/p.txt", "p"),
            ("software/onebot/bridge.json", "{}"),
            ("software/onebot/prompt.txt", "p"),
            ("software/x/bridge.json", "{}"),
            ("software/onebot/web/index.html", "x"),
            ("software/onebot/web/app.js", "x"),
            ("software/x/web/page.js", "x"),
            ("software/onebot/x/web/page.js", "x"),
        ] {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let mut found = BTreeMap::new();
        let walked = walk(&dir, "", &mut found);
        std::fs::remove_dir_all(&dir).unwrap();
        walked.unwrap();
        // 给人看的字、最上一层的模型资料、网页软件、软件包清单和出厂的预设、software/mermaid/、software/net/、人格目录的 persona.toml、
        // 群聊内核的出厂参数 software/onebot/defaults.toml、违规词表 moderation.txt、出厂的场所规则 venues.d/50-defaults.toml、桥自己的数
        // software/onebot/bridge.json、QQ 桥 WebUI 的页面 software/onebot/web/、视图投影的工具分类 core/view.json 不登记，别的照查（别处叫 models、web、mermaid、net 的
        // 目录、别处的 defaults.toml、moderation.txt 和 bridge.json、别的包的 web/、venues.d/ 里别的文件、onebot 包里别的文件照查：只有正好
        // 这几处才豁免）。
        assert_eq!(
            found.keys().collect::<Vec<_>>(),
            [
                "core/a.txt",
                "core/models/m.txt",
                "core/packages/p.txt",
                "core/presets/p.txt",
                "core/web/w.txt",
                "core/x/view.json",
                "personas/x/prompts/persona.md",
                "personas/x/prompts/persona.toml",
                "software/onebot/judge/system.txt",
                "software/onebot/prompt.txt",
                "software/onebot/venues.d/60-more.toml",
                "software/onebot/x/web/page.js",
                "software/x/bridge.json",
                "software/x/defaults.toml",
                "software/x/mermaid/not_special_here.json",
                "software/x/moderation.txt",
                "software/x/net/not_special_here.json",
                "software/x/tools/t.json",
                "software/x/web/page.js",
            ]
        );
    }
}
