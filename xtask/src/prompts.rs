//! 「给模型看的字」那一页（`docs/blueprint/prompts.md`，施工 4-9 三补）：照登记簿（`docs/designs/26-提示词.md` 第十节）
//! 和 `resources/` 生成。按进到请求的哪里分组，组照在登记簿里第一次出现的先后，组里照登记簿的先后；每一份写什么时候
//! 加进来、多少 token、为什么加、指纹，下面是原文。
//!
//! `cargo xtask prompts` 写这一页；`cargo xtask check` 重算一遍，和仓库里的不一样就算没过，算在「文档」那一项里。

use std::collections::BTreeMap;
use std::path::Path;

use crate::{drawing, ledger};

/// 这一页在仓库里的位置。
pub const PATH: &str = "docs/blueprint/prompts.md";

/// 登记簿里的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    /// `resources/` 下的路径。
    pub(crate) file: String,
    /// 进到哪。
    pub(crate) place: String,
    /// 什么时候加进来。
    pub(crate) when: String,
    /// 多少 token。
    pub(crate) tokens: String,
    /// 为什么加。
    pub(crate) why: String,
    /// 指纹。
    pub(crate) fingerprint: String,
}

/// 照仓库里的登记簿、资源生成这一页。
///
/// # Errors
///
/// 登记簿读不了、读不出来；登记了的资源读不了、不是 UTF-8。
pub fn render(root: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(root.join(ledger::PATH))
        .map_err(|e| format!("读不了登记簿 {}：{e}", ledger::PATH))?;
    let rows = rows(&text)?;
    let mut texts = BTreeMap::new();
    for row in &rows {
        let path = root.join(ledger::RESOURCES).join(&row.file);
        let bytes =
            std::fs::read(&path).map_err(|e| format!("读不了 resources/{}：{e}", row.file))?;
        let text =
            String::from_utf8(bytes).map_err(|_| format!("resources/{} 不是 UTF-8", row.file))?;
        texts.insert(row.file.clone(), text);
    }
    Ok(page(&rows, &texts))
}

/// 查：照资源目录、登记簿生成的，和仓库里的这一页一字不差。
pub fn check(root: &Path) -> Vec<String> {
    let expected = match render(root) {
        Ok(page) => page,
        Err(e) => return vec![e],
    };
    match std::fs::read_to_string(root.join(PATH)) {
        Ok(actual) if actual == expected => Vec::new(),
        Ok(_) => vec![format!(
            "{PATH} 和 resources/、登记簿对不上：这一页是生成的，跑 cargo xtask prompts 重新生成"
        )],
        Err(e) => vec![format!("读不了 {PATH}：{e}；跑 cargo xtask prompts 生成")],
    }
}

/// 生成这一页，写进仓库。
///
/// # Errors
///
/// 生成不了（见 [`render`]）；写不进去。
pub fn write(root: &Path) -> Result<(), String> {
    let page = render(root)?;
    std::fs::write(root.join(PATH), page).map_err(|e| format!("写不进 {PATH}：{e}"))
}

/// 登记簿那一节的表，每一行六格都要。
pub(crate) fn rows(text: &str) -> Result<Vec<Row>, String> {
    let section = drawing::section(text, ledger::SECTION)?;
    let tables = drawing::tables(&section);
    let table = drawing::find_table(&tables, &ledger::HEADER)?;
    table
        .rows
        .iter()
        .map(|row| {
            let [file, place, when, tokens, why, fingerprint] = row.as_slice() else {
                return Err(format!("登记簿的每一行要有 6 格：{}", row.join(" | ")));
            };
            let quoted = |cell: &String| {
                drawing::backticked(cell)
                    .into_iter()
                    .next()
                    .ok_or_else(|| format!("这一行的文件、指纹要写在反引号里：{}", row.join(" | ")))
            };
            Ok(Row {
                file: quoted(file)?,
                place: place.clone(),
                when: when.clone(),
                tokens: tokens.clone(),
                why: why.clone(),
                fingerprint: quoted(fingerprint)?,
            })
        })
        .collect()
}

/// 照登记簿的几行和它们的原文（照文件名查）写成一页。
pub(crate) fn page(rows: &[Row], texts: &BTreeMap<String, String>) -> String {
    let mut places: Vec<&str> = Vec::new();
    for row in rows {
        if !places.contains(&row.place.as_str()) {
            places.push(&row.place);
        }
    }
    let mut out = String::from(
        "## 给模型看的字\n\n\
         这一页是生成的：`cargo xtask prompts` 照 `resources/` 和登记簿（`docs/designs/26-提示词.md` 第十节）写出来，\
         别手改；`cargo xtask check` 查它和两边对得上（施工 4-9 三补）。给模型看的每一份字都在这里，按进到请求的哪里分组；\
         每一份写什么时候加进来、多少 token、为什么加、指纹，下面是原文。\n",
    );
    for place in places {
        out.push_str(&format!("\n### {place}\n"));
        for row in rows.iter().filter(|row| row.place == place) {
            let text = texts.get(&row.file).map_or("", String::as_str);
            out.push_str(&format!(
                "\n#### `{}`\n\n- 什么时候加进来：{}\n- token：{}\n- 为什么加：{}\n- 指纹：`{}`\n\n",
                row.file, row.when, row.tokens, row.why, row.fingerprint
            ));
            out.push_str(&fenced(&row.file, text));
        }
    }
    out
}

/// 原文放进围栏：围栏比原文里最长的一串反引号长，至少三个；去掉原文末尾的一个换行。
fn fenced(file: &str, text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat((longest + 1).max(3));
    let language = if file.ends_with(".json") {
        "json"
    } else {
        "text"
    };
    let body = text.strip_suffix('\n').unwrap_or(text);
    format!("{fence}{language}\n{body}\n{fence}\n")
}

#[cfg(test)]
mod tests;
