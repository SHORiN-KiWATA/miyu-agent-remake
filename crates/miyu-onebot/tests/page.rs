//! 后台页的文件（施工 O-28 上，`onebot.md` 第一条「后台页」第 3、4 条，「施工时定的」第 159、162 条）：页面没有构建工具、没有
//! 测试框架，守住最容易坏的几样。每一份 `.js` 照模块过 `node --check`（机器上没有 `node` 的跳过）；`texts.js` 里
//! `export const TEXTS = ` 后面是一段 JSON，三种语言的键一样；脚本里 `say('…')` 用到的编号都写了；入口照模块载 `app.js`。

use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{Map, Value};

use crate::support::resources;

/// 出厂的后台页目录。
fn page_dir() -> PathBuf {
    resources().join("packages/onebot/page")
}

/// 页面目录里每一份 `.js`：（文件名，内容）。
fn scripts() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = std::fs::read_dir(page_dir())
        .expect("有页面目录")
        .map(|entry| entry.expect("读得了").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "js"))
        .map(|path| {
            let name = path
                .file_name()
                .expect("有名字")
                .to_string_lossy()
                .into_owned();
            (name, std::fs::read_to_string(&path).expect("读得了"))
        })
        .collect();
    found.sort();
    found
}

/// `texts.js` 里的字：`export const TEXTS = ` 后面到最后的 `;` 是一段 JSON。交回语言 → 那一份。
fn texts() -> Map<String, Value> {
    let source = std::fs::read_to_string(page_dir().join("texts.js")).expect("读得了");
    // 照行首的那一处切：前面的注释里也提到这几个字。
    let (_, json) = source
        .split_once("\nexport const TEXTS =")
        .expect("有一行以 export const TEXTS = 开头");
    let json = json.trim_end().strip_suffix(';').expect("以 ; 结尾");
    let parsed: Value = serde_json::from_str(json).expect("是一段 JSON");
    parsed.as_object().expect("是对象").clone()
}

/// 一种语言的键。
fn keys(texts: &Map<String, Value>, language: &str) -> BTreeSet<String> {
    texts[language]
        .as_object()
        .unwrap_or_else(|| panic!("没有 {language}"))
        .keys()
        .cloned()
        .collect()
}

#[test]
fn every_script_parses_as_a_module() {
    let scripts = scripts();
    assert!(
        scripts.iter().any(|(name, _)| name == "app.js"),
        "{scripts:?}"
    );
    for (name, source) in scripts {
        let spawned = Command::new("node")
            .args(["--check", "--input-type=module"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("这台机器上没有 node，跳过页面脚本的语法检查");
                return;
            }
            Err(error) => panic!("起不来 node：{error}"),
        };
        child
            .stdin
            .take()
            .expect("接了管道")
            .write_all(source.as_bytes())
            .expect("写得进");
        let checked = child.wait_with_output().expect("等得到");
        assert!(
            checked.status.success(),
            "{name}：{}",
            String::from_utf8_lossy(&checked.stderr)
        );
    }
}

#[test]
fn the_three_languages_have_the_same_texts() {
    let texts = texts();
    assert_eq!(
        texts.keys().map(String::as_str).collect::<Vec<_>>(),
        ["en", "ja", "zh"]
    );
    let zh = keys(&texts, "zh");
    assert!(!zh.is_empty());
    assert_eq!(keys(&texts, "en"), zh);
    assert_eq!(keys(&texts, "ja"), zh);
    for (language, said) in &texts {
        for (key, text) in said.as_object().expect("是对象") {
            assert!(
                text.as_str().is_some_and(|text| !text.trim().is_empty()),
                "{language} {key}：{text}"
            );
        }
    }
}

#[test]
fn every_text_the_scripts_say_is_written() {
    let written = keys(&texts(), "zh");
    let mut said = BTreeSet::new();
    for (name, source) in scripts() {
        let mut rest = source.as_str();
        while let Some(at) = rest.find("say('") {
            rest = &rest[at + 5..];
            let end = rest
                .find('\'')
                .unwrap_or_else(|| panic!("{name}：引号没收"));
            said.insert(rest[..end].to_string());
        }
    }
    assert!(said.len() > 10, "{said:?}");
    let missing: Vec<_> = said.difference(&written).collect();
    assert!(missing.is_empty(), "texts.js 里没有：{missing:?}");
}

#[test]
fn the_entry_loads_the_app_as_a_module() {
    let index = std::fs::read_to_string(page_dir().join("index.html")).expect("读得了");
    assert!(
        index.contains(r#"<script type="module" src="app.js"></script>"#),
        "{index}"
    );
}
