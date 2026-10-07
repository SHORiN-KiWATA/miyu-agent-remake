//! 生成参考文件（`docs/blueprint/config.md`「怎么走」第一条第 10 条，G10）：全部配置项和默认值，给人翻着看，核心
//! 不读它。
//!
//! 开头两行说明它是生成的、改它没有用、要改的写在哪。接着键照表分开，表照名字的字母先后，表里的项也照字母先后：
//! 每一项先两行注释（名字和说明；能写什么、能放在哪几层、什么时候生效），再一行 `键 = 默认值`。没有默认值的（施工 8-6）
//! 这一行写成注释 `# 键 =`。人起的名字那一段写成带引号的占位：`[providers."<id>"]`，照样是读得懂的 TOML。表和表之间、
//! 项和项之间空一行，最后一个换行。

use crate::item::Item;
use crate::words::{self, Missing, Words};

/// 全部项的参考文件，字照 `words` 那一种语言。
///
/// # Errors
///
/// 要用的字缺了（资源目录装得不全）。
pub fn render(items: &[Item], words: &dyn Words) -> Result<String, Missing> {
    let mut text = String::new();
    comment(
        &mut text,
        &words::sentence(words, "config/reference-header", &[])?,
    );
    comment(
        &mut text,
        &words::sentence(words, "config/reference-where", &[])?,
    );
    let mut sorted: Vec<(String, &str, &Item)> = items
        .iter()
        .map(|item| {
            let (table, name) = item.key.rsplit_once('.').unwrap_or(("", item.key));
            let table: Vec<&str> = table.split('.').filter(|part| !part.is_empty()).collect();
            (crate::key::join(&table), name, item)
        })
        .collect();
    sorted.sort_by(|(a, x, _), (b, y, _)| (a, x).cmp(&(b, y)));
    let mut current = None;
    for (table, name, item) in sorted {
        text.push('\n');
        if current.as_ref() != Some(&table) {
            if !table.is_empty() {
                text.push_str(&format!("[{table}]\n"));
            }
            current = Some(table);
        }
        let said = words::item(words, item.key)?;
        comment(
            &mut text,
            &words::sentence(
                words,
                "config/reference-item",
                &[("name", &said.name), ("description", &said.description)],
            )?,
        );
        comment(&mut text, &words::facts(words, item)?);
        match &item.default {
            Some(default) => text.push_str(&format!(
                "{} = {}\n",
                crate::key::join(&[name]),
                default.toml()
            )),
            None => text.push_str(&format!("# {} =\n", crate::key::join(&[name]))),
        }
    }
    Ok(text)
}

/// 写成注释：每一行前面加 `# `。字里带换行的，每一行都加，不会漏出一行不是注释的。
fn comment(text: &mut String, said: &str) {
    for line in said.lines() {
        text.push_str("# ");
        text.push_str(line);
        text.push('\n');
    }
}

#[cfg(test)]
mod tests;
