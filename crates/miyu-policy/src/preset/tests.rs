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
    assert_eq!(
        file.name.as_ref().and_then(|name| name.pick("zh")),
        Some("开发")
    );
    assert!(file.summary.is_some());
    assert_eq!(
        file,
        read(&DEV.replace("default_persona = \"engineer\"\n", "")).unwrap(),
        "撤掉了的默认人格当没写（施工 P-4 上）"
    );
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
    assert_eq!(
        file.name.as_ref().and_then(|name| name.pick("en")),
        Some("我的开发"),
        "名字写了的整格换掉，没写的语言不再沿用"
    );
    assert_eq!(file.unlisted, Some(Unlisted::On));
    assert_eq!(file.software.get("memory"), Some(&true), "逐个键盖");
    assert_eq!(file.software.get("net"), Some(&true));
    assert_eq!(
        file.tools_off,
        BTreeSet::from(["shell".to_string(), "trash".to_string()]),
        "关掉的工具叠在一起"
    );
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
    assert_eq!(wrong("[preset]\nname = 3\n"), (Code::NotPhrases, Some(2)));
    assert_eq!(
        wrong("[preset]\nname = { fr = \"Dév\" }\n"),
        (Code::UnknownLanguage, Some(2))
    );
    assert_eq!(
        wrong("[preset]\nsummary = { en = \" \" }\n"),
        (Code::EmptyPhrase, Some(2))
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
    assert_eq!(Code::ALL.len(), 12);
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

#[test]
fn old_style_names_keep_the_digest_they_had() {
    // 名字、说明照以前的写法算，以前造的快照照旧对得上（`PresetFile::digest`）。默认人格撤了（施工 P-4 上），照没写算：
    // 这一份写了它，指纹是以前的写法里不写它的那一个（另用 Python 照以前的写法独立算过，P-3 补钉的写了它的是 f91563cc…）；
    // 空的那一份本来就没写，指纹不变。
    let dev = read(DEV).unwrap();
    assert_eq!(
        dev.digest().as_str(),
        "sha256:89703aaa280c2505d05919571905728248c06efd0a9476326d15b7681e41d9dd"
    );
    let empty = PresetFile::default();
    assert_eq!(
        empty.digest().as_str(),
        "sha256:49ff2847b261e037c53a2873ddd7ca9ccf1267bb9f6dc134bd3c7889896594cb"
    );
}

/// P-3 上那几个小时里建的预设写着 `base`（施工 P-3 再补）、撤掉了的默认人格（施工 P-4 上）：认出来当没写，写成什么样都
/// 不算写错，别的照读。
#[test]
fn a_base_written_by_p3_is_read_as_unwritten() {
    let plain = read("[preset]\nname = \"我的\"\n").unwrap();
    for text in [
        "[preset]\nbase = \"full\"\nname = \"我的\"\n",
        "[preset]\nname = \"我的\"\nbase = { a = 1 }\n",
        "[preset]\ndefault_persona = \"Miyu\"\nname = \"我的\"\n",
        "[preset]\nname = \"我的\"\ndefault_persona = 1\n",
    ] {
        assert_eq!(read(text).unwrap(), plain, "{text:?}");
    }
    assert_eq!(
        wrong("[software]\nbase = \"x\"\n").0,
        Code::NotBool,
        "别的表里照旧"
    );
}
