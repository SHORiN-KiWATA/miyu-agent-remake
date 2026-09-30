//! `web.human`：给人看的字（工具的显示名、对象是哪个参数、结果那一句的模板）。页面读不到本机的资源目录，桥读好了给
//! （蓝图 `web.md`「时间线的数」第 5 条；TUI 演示照 `MIYU_RESOURCES` 自己读）。
//!
//! 读法照 `miyu-store` 的 `human.rs`：内核的一份在 `core/human/`，每个软件包各一份在 `software/<包>/human/`，哪一份
//! 没有这种语言照英文；说法的编号前面加上它在哪（`core/…`、`software/<包>/…`）。模板原样给，页面照 `Human::say`
//! 的规矩换字段。核心的网页模块做出来以后由它给，删掉这一份。

use std::path::Path;

use serde_json::{Map, Value, json};

use miyu_store::env::Env;
use miyu_store::human::{FALLBACK, Human};
use miyu_store::resources::ResourceRoot;

/// 读 `language` 这一种：`{"tools": {工具名: 样子}, "said": {说法的编号: 模板}}`。
///
/// # Errors
///
/// 找不到资源目录；有一份读得到、却读不懂：交回一句说清楚的话。
pub fn load(language: &str) -> Result<Value, String> {
    let root = ResourceRoot::locate(&Env::current()).map_err(|e| e.to_string())?;
    // 先照 TUI、`miyu ask` 的读法验一遍：读不懂的（模板坏了、多了格子）照它的话报出来
    Human::load(&root, language).map_err(|e| e.to_string())?;
    let mut out = (Map::new(), Map::new());
    add(&root.path().join("core"), "core", language, &mut out)?;
    let software = root.path().join("software");
    let mut packages: Vec<String> = match std::fs::read_dir(&software) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    };
    packages.sort();
    for package in packages {
        add(&software.join(&package), &format!("software/{package}"), language, &mut out)?;
    }
    Ok(json!({"tools": out.0, "said": out.1}))
}

/// 读 `dir` 下 `human/` 里 `language` 那一份，没有就读英文那一份，接进 `(tools, said)`；说法的编号前面加上 `prefix`。
fn add(dir: &Path, prefix: &str, language: &str, (tools, said): &mut (Map<String, Value>, Map<String, Value>)) -> Result<(), String> {
    let found = [language, FALLBACK].iter().find_map(|language| {
        let file = dir.join("human").join(format!("{language}.json"));
        std::fs::read_to_string(&file).ok().map(|text| (file, text))
    });
    let Some((file, text)) = found else {
        return Ok(());
    };
    let parsed: Value = serde_json::from_str(&text).map_err(|e| format!("{}：{e}", file.display()))?;
    if let Some(each) = parsed["tools"].as_object() {
        tools.extend(each.clone());
    }
    if let Some(each) = parsed["said"].as_object() {
        for (key, template) in each {
            said.insert(format!("{prefix}/{key}"), template.clone());
        }
    }
    Ok(())
}
