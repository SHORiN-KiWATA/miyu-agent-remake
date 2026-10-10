//! 给人看的字不写「她」（施工 O-29，`onebot.md` 第一条「给人看的字」；项目主人 2026-10-10 定）：接入QQ 的清单（名字、说明、
//! 配置项的字）、后台页的字、桥说给人听的字、出厂参数文件里的说明，都不用「她」「彼女」「she」「her」，写「AI」或者换个说法。
//! 给模型看的字（判官的模板、事实、工具说明）不在这一条里。

use std::path::Path;

/// 源码树里的资源目录。
fn resources() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 给人看的那几份，相对资源目录。
const HUMAN_FACING: [&str; 6] = [
    "packages/onebot/package.toml",
    "packages/onebot/page/texts.js",
    "software/onebot/human/zh.json",
    "software/onebot/human/en.json",
    "software/onebot/human/ja.json",
    "software/onebot/defaults.toml",
];

/// 一行里有没有用代词说 AI：中文、日文照字，英文照整词（不分大小写）。
fn pronoun(line: &str) -> bool {
    if line.contains('她') || line.contains("彼女") {
        return true;
    }
    line.split(|c: char| !c.is_ascii_alphabetic()).any(|word| {
        matches!(
            word.to_ascii_lowercase().as_str(),
            "she" | "her" | "hers" | "herself"
        )
    })
}

#[test]
fn human_facing_texts_do_not_call_the_ai_she() {
    let mut found = Vec::new();
    for file in HUMAN_FACING {
        let text = std::fs::read_to_string(resources().join(file)).expect("出厂的文件读得出来");
        for (number, line) in text.lines().enumerate() {
            if pronoun(line) {
                found.push(format!("{file}:{}: {line}", number + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "给人看的字里还有代词：\n{}",
        found.join("\n")
    );
}

#[test]
fn the_check_catches_each_form() {
    for line in [
        "和她说话",
        "彼女に",
        "Talk with her",
        "while She sleeps",
        "HERSELF",
    ] {
        assert!(pronoun(line), "{line}");
    }
    for line in ["here", "there", "shell", "the AI", "AI と話す"] {
        assert!(!pronoun(line), "{line}");
    }
}
