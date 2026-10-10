//! 预设文件的读法（施工 P-2 上）：每一格读对、三层没写的照默认；逐格叠；每一种写错写明第几行。

use super::*;
use crate::features::Features;

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
    assert_eq!(Code::ALL.len(), 13);
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
    assert!(
        dev.keeps("files", "basesystem", "read"),
        "功能没写，照包开着"
    );
    assert!(!dev.keeps("commands", "basesystem", "shell"), "单件关掉的");
    assert!(!dev.keeps("memory", "memory", "remember"), "包没开的");
}

/// 装了的几个功能：基础系统两个，人格记忆、人设防失忆提醒、QQ 工具各一个。
fn installed() -> Features {
    let feature = |id: &str, package: &str| crate::features::Feature {
        id: id.to_string(),
        package: package.to_string(),
        tools: Vec::new(),
    };
    Features::new(vec![
        feature("files", "basesystem"),
        feature("commands", "basesystem"),
        feature("memory", "memory"),
        feature("roleplay", "roleplay"),
        feature("qq", "onebot"),
    ])
}

/// 照功能开关（施工 F-3 上，设计 30 第四节）：`[features]` 写了的照写的，压过以前的 `[software]`；没写的照 `[software]` 的功能
/// 编号、再照包的编号；都没写的照 `unlisted`。
#[test]
fn features_open_as_written_and_fall_back_to_software_then_unlisted() {
    let file = read(
        "[preset]\nunlisted = \"off\"\n\n[features]\ncommands = false\nqq = true\n\n[software]\nbasesystem = true\nmemory = true\n",
    )
    .unwrap();
    assert!(file.opens_in("files", "basesystem"), "照包开着");
    assert!(
        !file.opens_in("commands", "basesystem"),
        "功能写了 false，压过包"
    );
    assert!(file.opens_in("qq", "onebot"), "功能写了 true，包没写");
    assert!(file.opens("memory"), "和包同编号的照 [software]");
    assert!(!file.opens("roleplay"), "都没写的照 unlisted = off");
    let legacy = read("[software]\ncommands = false\nbasesystem = true\n").unwrap();
    assert!(
        !legacy.opens_in("commands", "basesystem"),
        "[software] 里写功能的编号，先于包的编号"
    );
    let lower = read("[features]\nfiles = false\n").unwrap();
    let over = read("[features]\nfiles = true\n").unwrap().over(lower);
    assert!(
        over.opens_in("files", "basesystem"),
        "上面一层的功能盖下面的"
    );
}

#[test]
fn chosen_lists_installed_features_that_are_off_in_order() {
    let chosen = Chosen::new("dev".to_string(), read(DEV).unwrap(), &installed());
    assert_eq!(
        chosen.off,
        ["memory", "qq", "roleplay"],
        "照编号排、不重复；开着的不算；包没写进 [software] 的照 unlisted"
    );
    let full = Chosen::new("full".to_string(), PresetFile::default(), &installed());
    assert!(full.off.is_empty());
    let kept = Chosen::new("dev".to_string(), read(DEV).unwrap(), &installed())
        .keeping_memory(true, &installed());
    assert_eq!(kept.off, ["qq", "roleplay"], "记忆照开会话时的");
    assert_eq!(kept.digest, chosen.digest, "指纹照找到的那一份");
}

/// 以前的快照记的是包的编号（施工 F-3 上）：照现在的功能读一样的，算同一回事，不换快照。
#[test]
fn an_old_pin_with_package_ids_means_the_same() {
    let new = Chosen::new("dev".to_string(), read(DEV).unwrap(), &installed()).pin();
    let mut old = new.clone();
    old.off = ["memory", "onebot", "roleplay"].map(String::from).to_vec();
    assert!(old.means_the_same(&new, &installed()));
    let mut other = old.clone();
    other.off = ["memory"].map(String::from).to_vec();
    assert!(!other.means_the_same(&new, &installed()), "少关了的不一样");
    let mut moved = new.clone();
    moved.digest = None;
    assert!(!moved.means_the_same(&new, &installed()), "指纹不一样");
}

/// 写了 `[features]` 的指纹把它算进去；没写的照以前的几格算（施工 F-3 上）。
#[test]
fn features_change_the_digest_only_when_written() {
    let dev = read(DEV).unwrap();
    let mut with = dev.clone();
    with.features.insert("files".to_string(), false);
    assert_ne!(with.digest(), dev.digest());
}

#[test]
fn features_and_software_are_read_into_their_own_tables() {
    let file = read("[features]\nfiles = false\n\n[software]\nmemory = true\n").unwrap();
    assert_eq!(file.features.get("files"), Some(&false));
    assert!(!file.software.contains_key("files"));
    assert_eq!(file.software.get("memory"), Some(&true));
    assert!(!file.features.contains_key("memory"));
}

#[test]
fn a_feature_id_is_written_like_a_package_id() {
    let problem = read("[features]\nFiles = true\n").unwrap_err();
    assert_eq!((problem.code, problem.line), (Code::BadFeature, Some(2)));
    let problem = read("[features]\nfiles = \"yes\"\n").unwrap_err();
    assert_eq!((problem.code, problem.line), (Code::NotBool, Some(2)));
    assert_eq!(problem.detail, "features.files");
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
