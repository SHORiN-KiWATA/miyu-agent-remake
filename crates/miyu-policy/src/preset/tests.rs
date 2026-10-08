//! 预设文件的读法（施工 P-2 上）：每一格读对、三层没写的照默认；逐格叠；每一种写错写明第几行。

use super::*;

const DEV: &str = r#"[preset]
name = { en = "Dev", zh = "开发" }
summary = { en = "Only what coding needs" }
default_persona = "engineer"
unlisted = "off"

[software]
basesystem = true
net = true
memory = false

[tools]
shell = false
"#;

fn wrong(text: &str) -> (Code, Option<usize>) {
    let problem = read(text).expect_err("应该读不成");
    (problem.code, problem.line)
}

#[test]
fn every_field_reads() {
    let file = read(DEV).unwrap();
    assert_eq!(file.name.get("zh").map(String::as_str), Some("开发"));
    assert_eq!(file.summary.len(), 1);
    assert_eq!(file.default_persona.as_deref(), Some("engineer"));
    assert_eq!(file.unlisted, Some(Unlisted::Off));
    assert_eq!(
        file.software,
        BTreeMap::from([
            ("basesystem".to_string(), true),
            ("memory".to_string(), false),
            ("net".to_string(), true),
        ])
    );
    assert_eq!(file.tools_off, BTreeSet::from(["shell".to_string()]));
}

#[test]
fn an_empty_file_has_nothing_and_unlisted_is_on() {
    let file = read("").unwrap();
    assert_eq!(file, PresetFile::default());
    assert_eq!(file.unlisted(), Unlisted::On, "三层都没写的照 on");
    assert_eq!(read(DEV).unwrap().unlisted(), Unlisted::Off);
    assert_eq!(
        (Unlisted::On.as_str(), Unlisted::Off.as_str()),
        ("on", "off")
    );
}

#[test]
fn an_upper_layer_overrides_field_by_field() {
    let lower = read(DEV).unwrap();
    let upper = read(
        "[preset]\nname = { zh = \"我的开发\" }\nunlisted = \"on\"\n\n[software]\nmemory = true\n\n[tools]\ntrash = false\n",
    )
    .unwrap();
    let file = upper.over(lower);
    assert_eq!(file.name.get("zh").map(String::as_str), Some("我的开发"));
    assert_eq!(
        file.name.get("en").map(String::as_str),
        Some("Dev"),
        "没写的语言沿用"
    );
    assert_eq!(
        file.default_persona.as_deref(),
        Some("engineer"),
        "没写的沿用"
    );
    assert_eq!(file.unlisted, Some(Unlisted::On));
    assert_eq!(file.software.get("memory"), Some(&true), "逐个键盖");
    assert_eq!(file.software.get("net"), Some(&true));
    assert_eq!(
        file.tools_off,
        BTreeSet::from(["shell".to_string(), "trash".to_string()]),
        "关掉的工具叠在一起"
    );
    let persona = read("[preset]\ndefault_persona = \"miyu\"\n")
        .unwrap()
        .over(read(DEV).unwrap());
    assert_eq!(persona.default_persona.as_deref(), Some("miyu"));
}

#[test]
fn every_wrong_one_says_which_line() {
    assert_eq!(wrong("[preset\n").0, Code::Syntax);
    assert_eq!(
        wrong("[preset]\n\n[colors]\n"),
        (Code::UnknownTable, Some(3))
    );
    assert_eq!(wrong("preset = 1\n"), (Code::NotATable, Some(1)));
    assert_eq!(
        wrong("[preset]\ncolor = \"red\"\n"),
        (Code::UnknownKey, Some(2))
    );
    assert_eq!(
        wrong("[preset]\nname = \"Dev\"\n"),
        (Code::NotPhrases, Some(2))
    );
    assert_eq!(
        wrong("[preset]\nname = { fr = \"Dév\" }\n"),
        (Code::UnknownLanguage, Some(2))
    );
    assert_eq!(
        wrong("[preset]\nsummary = { en = \" \" }\n"),
        (Code::EmptyPhrase, Some(2))
    );
    assert_eq!(
        wrong("[preset]\ndefault_persona = \"Miyu\"\n"),
        (Code::BadPersona, Some(2))
    );
    assert_eq!(
        wrong("[preset]\ndefault_persona = 1\n"),
        (Code::BadPersona, Some(2))
    );
    assert_eq!(
        wrong("[preset]\nunlisted = \"maybe\"\n"),
        (Code::BadUnlisted, Some(2))
    );
    assert_eq!(wrong("software = 1\n"), (Code::NotATable, Some(1)));
    assert_eq!(
        wrong("[software]\nNet = true\n"),
        (Code::BadSoftware, Some(2))
    );
    assert_eq!(
        wrong("[software]\nnet = \"yes\"\n"),
        (Code::NotBool, Some(2))
    );
    assert_eq!(wrong("tools = 1\n"), (Code::NotATable, Some(1)));
    assert_eq!(
        wrong("[tools]\n\"a b\" = false\n"),
        (Code::BadTool, Some(2))
    );
    assert_eq!(
        wrong("[tools]\nshell = true\n"),
        (Code::NotFalse, Some(2)),
        "单件打开先不做"
    );
    assert_eq!(wrong("[tools]\nshell = 0\n"), (Code::NotFalse, Some(2)));
}

#[test]
fn tool_names_take_letters_digits_dashes_and_underscores() {
    let file = read("[tools]\nweb_fetch = false\nmcp-x = false\nRead2 = false\n").unwrap();
    assert_eq!(file.tools_off.len(), 3);
    let long = format!("[tools]\n{} = false\n", "a".repeat(65));
    assert_eq!(wrong(&long).0, Code::BadTool);
    assert_eq!(
        wrong("[preset]\nbase = \"Dev\"\n"),
        (Code::BadBase, Some(2))
    );
}

#[test]
fn a_problem_reads_as_a_line_and_a_sentence() {
    let problem = read("[preset]\nunlisted = \"maybe\"\n").unwrap_err();
    assert_eq!(problem.detail, "preset.unlisted");
    assert_eq!(problem.to_string(), "2: preset.unlisted must be on or off");
    let no_line = Problem {
        line: None,
        code: Code::Syntax,
        detail: String::new(),
        message: "broken".to_string(),
    };
    assert_eq!(no_line.to_string(), "broken");
    assert_eq!(Code::ALL.len(), 14);
}

#[test]
fn software_opens_as_written_and_unlisted_ones_follow_unlisted() {
    let dev = read(DEV).unwrap();
    assert!(dev.opens("basesystem"));
    assert!(!dev.opens("memory"), "写了 false");
    assert!(!dev.opens("roleplay"), "没写的照 unlisted = off");
    let full = read("[software]\nmemory = false\n").unwrap();
    assert!(full.opens("roleplay"), "没写的照 unlisted，几层都没写是 on");
    assert!(!full.opens("memory"));
    assert!(dev.keeps("basesystem", "read"));
    assert!(!dev.keeps("basesystem", "shell"), "单件关掉的");
    assert!(!dev.keeps("memory", "remember"), "包没开的");
}

#[test]
fn chosen_lists_installed_software_that_is_off_in_order() {
    let chosen = Chosen::new(
        "dev".to_string(),
        read(DEV).unwrap(),
        ["roleplay", "basesystem", "memory", "net", "memory"],
    );
    assert_eq!(
        chosen.off,
        ["memory", "roleplay"],
        "照编号排、不重复；开着的不算"
    );
    let full = Chosen::new("full".to_string(), PresetFile::default(), ["memory"]);
    assert!(full.off.is_empty());
}
