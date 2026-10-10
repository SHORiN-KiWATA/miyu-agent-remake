//! 以前写 `kind` 的清单改成新的（施工 F-8 上补，设计 `31-软件包.md` 第二节第 1 条、第七节第 2 条）：包带了什么看有哪几张表，
//! `[package] kind` 不再写。家目录里装过的照它改一次；出厂的在仓库里改好。只改文本：去掉 `kind` 那一行，种类是 `ui`、
//! `process`、`builtin`、`worker`、`mascot` 而清单里还没有那一张表的，在末尾补一张空的，别的一个字不动。

use toml_edit::Document;

/// 照以前写的种类要补的那张表。
fn table_of(kind: &str) -> Option<&'static str> {
    match kind {
        "ui" => Some("ui"),
        "process" => Some("process"),
        "builtin" => Some("builtin"),
        "worker" => Some("worker"),
        "mascot" => Some("mascot"),
        _ => None,
    }
}

/// `text` 写了 `[package] kind`、认得出种类、`kind` 单独占一行的，交回改好的；别的（没写、读不成、写的不是这几种）交回
/// 没有，原样不动，照写错了报。
pub fn without_kind(text: &str) -> Option<String> {
    let document = Document::parse(text).ok()?;
    let package = document.get("package")?.as_table()?;
    let (key, item) = package.get_key_value("kind")?;
    let table = table_of(item.as_str()?)?;
    let start = key.span()?.start;
    let end = item.span()?.end;
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |at| end + at + 1);
    let after = text[end..line_end].trim();
    if !text[line_start..start].trim().is_empty() || !(after.is_empty() || after.starts_with('#')) {
        return None;
    }
    let mut out = String::with_capacity(text.len() + 16);
    out.push_str(&text[..line_start]);
    out.push_str(&text[line_end..]);
    if document.get(table).is_none() {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("\n[{table}]\n"));
    }
    Some(out)
}

#[cfg(test)]
mod tests;
