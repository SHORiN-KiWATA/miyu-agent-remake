//! 给人看的字（施工 4-5 上）：内核给模型的每一句都有给人看的一句对着，要的字段不多于给模型的那一句；工具的
//! 显示名；找不到的语言照英文；换进去的字段去掉控制字符；读不懂的说是哪一份，没有的不算错。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_store::human::{Block, Human, clean};
use miyu_store::resources::ResourceRoot;

/// 源码树里的资源目录。
fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

fn load(language: &str) -> Human {
    Human::load(&ResourceRoot::at(resources()), language).expect("出厂的字读得出来")
}

/// 一个用完就删的临时资源目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-human-{}-{n}", std::process::id())))
    }

    fn file(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, text).expect("写得进");
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn every_kernel_sentence_has_one_for_people_in_both_languages() {
    for language in ["zh", "en"] {
        let words = load(language);
        for dir in ["tool-results", "permissions"] {
            let mut count = 0;
            for entry in std::fs::read_dir(resources().join("core").join(dir)).expect("在") {
                let path = entry.expect("读得了").path();
                let name = path
                    .file_stem()
                    .expect("有名字")
                    .to_string_lossy()
                    .into_owned();
                let key = format!("core/{dir}/{name}");
                let model = Template::parse(&std::fs::read_to_string(&path).expect("读得了"))
                    .expect("给模型的那一句写法对");
                let Some(fields) = words.fields(&key) else {
                    panic!("{language} 没有 {key}");
                };
                // 给人看的只能用给模型的那一句有的字段：内核只交得出那几个。
                for field in fields {
                    assert!(
                        model.fields().contains(&field),
                        "{language} {key} 要了 {field}"
                    );
                }
                count += 1;
            }
            assert!(count > 0, "{dir} 下有字");
        }
    }
}

#[test]
fn a_said_turns_into_words_in_the_language_asked_for() {
    let said = Said::new("core/tool-results/unattended");
    assert_eq!(
        load("zh").say(&said).as_deref(),
        Some("要确认，这里没人能确认")
    );
    assert_eq!(
        load("en").say(&said).as_deref(),
        Some("needs a confirmation nobody here can give")
    );
    let lines = Said::new("software/basesystem/read/lines").with("count", "37");
    assert_eq!(load("zh").say(&lines).as_deref(), Some("37 行"));
    // 没有这种语言的，照英文。
    assert_eq!(load("fr").say(&lines).as_deref(), Some("37 lines"));
    // 没有这一句、少了字段的：换不出来，头照状态写最泛的。
    assert_eq!(load("zh").say(&Said::new("core/nope")), None);
    assert_eq!(
        load("zh").say(&Said::new("software/basesystem/read/lines")),
        None
    );
}

#[test]
fn tools_have_a_name_and_the_argument_that_follows_it() {
    let zh = load("zh");
    let read = zh.tool("read").expect("有 read");
    assert_eq!(
        (read.name.as_str(), read.subject.as_deref()),
        ("读取", Some("file_path"))
    );
    assert_eq!(
        zh.tool("grep").map(|face| face.name.as_str()),
        Some("搜内容")
    );
    assert_eq!(
        load("en").tool("glob").map(|face| face.name.as_str()),
        Some("Find files")
    );
    assert!(zh.tool("nope").is_none());
    // 符号、下面那一块（施工 4-11）。
    assert_eq!(read.icon.as_deref(), Some("→"));
    assert_eq!(read.block, None);
    let shell = zh.tool("shell").expect("有 shell");
    assert_eq!(
        (shell.icon.as_deref(), shell.block),
        (Some("$"), Some(Block::Command))
    );
    assert_eq!(
        zh.tool("edit").and_then(|face| face.block),
        Some(Block::Edits)
    );
    // 跟语言无关的几格，两种语言写的一样；七件都有符号。
    let en = load("en");
    for tool in ["read", "glob", "grep", "write", "edit", "trash", "shell"] {
        let (zh, en) = (zh.tool(tool).expect(tool), en.tool(tool).expect(tool));
        assert_eq!(
            (&zh.subject, &zh.icon, zh.block),
            (&en.subject, &en.icon, en.block),
            "{tool}"
        );
        assert!(zh.icon.is_some(), "{tool}");
    }
}

#[test]
fn control_characters_in_fields_are_replaced() {
    assert_eq!(clean("a\u{1b}[31mb\nc"), "a\u{fffd}[31mb\u{fffd}c");
    assert_eq!(
        clean("路径 \"x\" <y>"),
        "路径 \"x\" <y>",
        "引号、尖括号照原样"
    );
    let said = Said::new("software/basesystem/common/missing-similar")
        .with("path", "x")
        .with("similar", "evil\u{1b}]52;c;AAAA\u{7}.txt");
    let words = load("en").say(&said).expect("换得出来");
    assert!(!words.chars().any(char::is_control), "{words:?}");
    // 引号、反斜杠照原样，不像给模型的那样转义。
    let quoted = Said::new("software/basesystem/common/missing-similar")
        .with("path", "x")
        .with("similar", r#"a"b\c.txt"#);
    assert_eq!(
        load("en").say(&quoted).as_deref(),
        Some(r#"no such file, did you mean a"b\c.txt"#)
    );
}

#[test]
fn a_broken_file_is_named_and_a_missing_one_is_fine() {
    let scratch = Scratch::new();
    // 什么都没有：没有字，不算错。
    let empty = Human::load(&ResourceRoot::at(&scratch.0), "zh").expect("没有的不算错");
    assert!(empty.tool("read").is_none());
    // 一个软件包只有英文：照英文。
    scratch.file(
        "software/pkg/human/en.json",
        r#"{"tools":{"t":{"name":"T"}},"said":{"x/y":"why {n}"}}"#,
    );
    let words = Human::load(&ResourceRoot::at(&scratch.0), "zh").expect("读得出来");
    assert_eq!(words.tool("t").map(|face| face.name.as_str()), Some("T"));
    assert_eq!(
        words
            .say(&Said::new("software/pkg/x/y").with("n", "1"))
            .as_deref(),
        Some("why 1")
    );
    // 读得到却读不懂：说是哪一份。
    scratch.file("core/human/zh.json", "{");
    let error = Human::load(&ResourceRoot::at(&scratch.0), "zh").expect_err("读不懂");
    assert!(error.file.ends_with("core/human/zh.json"), "{error}");
    scratch.file("core/human/zh.json", r#"{"said":{"a":"{nope"}}"#);
    let error = Human::load(&ResourceRoot::at(&scratch.0), "zh").expect_err("模板坏了");
    assert!(error.why.starts_with("a: "), "{error}");
    // 下面那一块只认两种。
    scratch.file(
        "core/human/zh.json",
        r#"{"tools":{"t":{"name":"T","block":"diff"}}}"#,
    );
    let error = Human::load(&ResourceRoot::at(&scratch.0), "zh").expect_err("不认识的块");
    assert!(error.why.contains("diff"), "{error}");
}
