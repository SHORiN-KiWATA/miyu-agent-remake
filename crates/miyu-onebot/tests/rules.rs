//! 场所规则和出厂数据（施工 O-21，`onebot.md` 第一条「场所规则和出厂数据」）：出厂的读得出、零问题；出厂、系统两份照文件名
//! 的先后套，系统同名的整份替换出厂的；系统的写错只丢那一部分，读不成的照空的用；出厂的写错读不出来、交回每一条问题；违规
//! 词表系统那一份替换出厂的。什么时候重读在 `reload.rs`。

use std::path::{Path, PathBuf};

use miyu_chat::{Entry, Origin, Source, Venue, VenueKind};
use miyu_config::Value;
use miyu_config::problem::Code;
use miyu_onebot::rules::{Factory, load};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::support::*;

/// 出厂的违规词表有几个词（`chat.md` 第二条第 12 条：旧版的 153 个）。
pub const FACTORY_WORDS: usize = 153;

/// `qq:group:<号>`。
pub fn group(number: &str) -> Venue {
    Venue::new("qq", VenueKind::Group, number).expect("合写法")
}

/// 系统的 `venues.d/<name>` 写成 `text`（字节）。
pub fn system_rule(root: &DataRoot, name: &str, text: impl AsRef<[u8]>) -> PathBuf {
    let dir = root.system().join("venues.d");
    std::fs::create_dir_all(&dir).expect("建得了");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("写得进");
    path
}

/// 系统的违规词表 `modules/onebot/moderation.txt` 写成 `text`（字节）。
pub fn system_words(root: &DataRoot, text: impl AsRef<[u8]>) -> PathBuf {
    let dir = root.system().join("modules").join("onebot");
    std::fs::create_dir_all(&dir).expect("建得了");
    let path = dir.join("moderation.txt");
    std::fs::write(&path, text).expect("写得进");
    path
}

/// 一项的值和来处。
fn entry(value: Value, source: Source, file: &str, rule: usize, line: usize) -> Entry {
    Entry {
        value,
        origin: Origin {
            source,
            file: file.to_string(),
            rule,
            line,
        },
    }
}

/// 字的值。
pub fn text(value: &str) -> Value {
    Value::Text(value.to_string().into())
}

/// 删掉临时目录；删不掉就留在临时目录里，不影响测试。
pub fn clean(dir: &Path) {
    if std::fs::remove_dir_all(dir).is_err() {
        // 留着。
    }
}

/// 一份临时的资源目录：照源码树抄来桥起来要的几份（清单、`bridge.json`、给人看的字、出厂的规则、参数、词表），交回目录
/// （在一个临时目录里，用完删它的上一级）。
pub fn copied_resources() -> PathBuf {
    let (dir, _) = temp_root();
    let copy = dir.join("resources");
    for file in [
        "packages/onebot.toml",
        "software/onebot/bridge.json",
        "software/onebot/human/en.json",
        "software/onebot/human/zh.json",
        "software/onebot/human/ja.json",
        "software/onebot/venues.d/50-defaults.toml",
        "software/onebot/defaults.toml",
        "software/onebot/moderation.txt",
    ] {
        let to = copy.join(file);
        std::fs::create_dir_all(to.parent().expect("有上一级")).expect("建得了");
        std::fs::copy(resources().join(file), &to).expect("抄得了");
    }
    // 判官的说明（施工 O-23 下）、给她看的事实的模板（施工 O-25 下）：整个目录抄过去。
    for dir in ["software/onebot/judge", "software/onebot/facts"] {
        let to = copy.join(dir);
        std::fs::create_dir_all(&to).expect("建得了");
        for entry in std::fs::read_dir(resources().join(dir)).expect("列得出") {
            let entry = entry.expect("读得了");
            std::fs::copy(entry.path(), to.join(entry.file_name())).expect("抄得了");
        }
    }
    copy
}

#[test]
fn the_factory_data_is_read_clean() {
    let (dir, root) = temp_root();
    let loaded = load(&factory(), &root);
    assert_eq!(loaded.problems, [], "出厂的、没有系统的：零问题");
    assert_eq!(loaded.keywords.len(), FACTORY_WORDS);
    let applied = loaded.at(&group("1"));
    let factory = |value, line| entry(value, Source::Factory, "50-defaults.toml", 1, line);
    assert_eq!(
        applied.resolved.entries.get("discipline"),
        Some(&factory(text("chatty"), 14))
    );
    assert_eq!(
        applied.resolved.entries.get("rate"),
        Some(&factory(text("5/300s"), 15))
    );
    assert_eq!(
        applied.resolved.entries.get("parallel"),
        Some(&factory(Value::Int(1), 16))
    );
    assert_eq!(applied.resolved.entries.len(), 3, "出厂的只写了三项");
    assert_eq!(
        applied.params.judge.retries, 1,
        "参数照出厂的 defaults.toml"
    );
    clean(&dir);
}

#[test]
fn system_files_go_with_the_factory_ones_by_file_name() {
    let (dir, root) = temp_root();
    // 排在出厂的前面：它设的 discipline 被出厂的盖掉，出厂没设的 allow 留着。
    system_rule(
        &root,
        "10-early.toml",
        "[[rule]]\nmatch = { kind = \"group\" }\ndiscipline = \"when-called\"\nallow = false\n",
    );
    // 排在后面：盖出厂的；参数只改写了的几项，只对群 1。
    system_rule(
        &root,
        "80-late.toml",
        "[[rule]]\nmatch = { group = [1] }\nrate = \"30/60s\"\nchatty = { probability = 80 }\njudge = { retries = 3 }\n",
    );
    let loaded = load(&factory(), &root);
    assert_eq!(loaded.problems, []);
    let one = loaded.at(&group("1"));
    let late = |value, line| entry(value, Source::System, "80-late.toml", 1, line);
    assert_eq!(
        one.resolved.entries.get("discipline"),
        Some(&entry(
            text("chatty"),
            Source::Factory,
            "50-defaults.toml",
            1,
            14
        ))
    );
    assert_eq!(
        one.resolved.entries.get("allow"),
        Some(&entry(
            Value::Bool(false),
            Source::System,
            "10-early.toml",
            1,
            4
        ))
    );
    assert_eq!(
        one.resolved.entries.get("rate"),
        Some(&late(text("30/60s"), 3))
    );
    assert_eq!(
        one.resolved.entries.get("chatty.probability"),
        Some(&late(Value::Int(80), 4))
    );
    assert_eq!(one.params.judge.retries, 3, "参数照规则改");
    let two = loaded.at(&group("2"));
    assert_eq!(
        two.resolved.entries.get("rate").map(|entry| &entry.value),
        Some(&text("5/300s")),
        "别的群照出厂的"
    );
    assert_eq!(two.params.judge.retries, 1);
    clean(&dir);
}

#[test]
fn a_system_file_with_the_same_name_replaces_the_factory_one() {
    let (dir, root) = temp_root();
    system_rule(&root, "50-defaults.toml", "[[rule]]\nparallel = 2\n");
    let entries = load(&factory(), &root).at(&group("1")).resolved.entries;
    assert_eq!(
        entries.into_iter().collect::<Vec<_>>(),
        [(
            "parallel",
            entry(Value::Int(2), Source::System, "50-defaults.toml", 1, 2)
        )],
        "出厂的那一份整个不读"
    );
    clean(&dir);
}

#[test]
fn only_rule_files_are_read() {
    let (dir, root) = temp_root();
    // 设一项出厂没设的：排在哪都看得出读没读。
    let rule = "[[rule]]\nallow = false\n";
    // 不以 `.toml` 结尾、以 `.` 开头（编辑器的锁文件）、是个目录的：都不是规则文件。
    system_rule(&root, "80-notes.txt", rule);
    system_rule(&root, "80-backup.toml~", rule);
    system_rule(&root, ".#80-lock.toml", rule);
    std::fs::create_dir_all(root.system().join("venues.d").join("80-dir.toml")).expect("建得了");
    let loaded = load(&factory(), &root);
    assert_eq!(loaded.problems, [], "不是规则文件的不读，也不报");
    assert_eq!(loaded.at(&group("1")).resolved.entries.get("allow"), None);
    system_rule(&root, "90-real.toml", rule);
    assert_eq!(
        load(&factory(), &root).at(&group("1")).resolved.entries["allow"].value,
        Value::Bool(false)
    );
    clean(&dir);
}

#[test]
fn a_system_mistake_drops_only_that_part_and_is_reported() {
    let (dir, root) = temp_root();
    // 写错的那一项丢掉，同一条规则里别的照收。
    system_rule(
        &root,
        "80-bad.toml",
        "[[rule]]\nmatch = { group = [1] }\nrate = \"abc\"\nparallel = 3\n",
    );
    // 写法不对的整份不用；超过 1 MiB、不是 UTF-8 的不读：都只报那一份。
    system_rule(&root, "85-syntax.toml", "[[rule]\nparallel = 2\n");
    let mut big = b"[[rule]]\nparallel = 2\n".to_vec();
    big.resize(1024 * 1024 + 1, b'#');
    system_rule(&root, "70-big.toml", big);
    // 读不成的排在写错的中间：问题照文件名排，不照先读后查。
    system_rule(
        &root,
        "95-gbk.toml",
        b"[[rule]]\nparallel = 2\n# \xc4\xe3\n",
    );
    let loaded = load(&factory(), &root);
    let entries = loaded.at(&group("1")).resolved.entries;
    assert_eq!(entries["parallel"].value, Value::Int(3), "别的照收");
    assert_eq!(
        entries["rate"].origin.source,
        Source::Factory,
        "写错的不盖出厂的"
    );
    let seen = loaded
        .problems
        .iter()
        .map(|problem| {
            (
                problem.code,
                problem.source,
                problem.file.as_str(),
                problem.rule,
                problem.key.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        seen,
        [
            (Code::TooBig, Source::System, "70-big.toml", None, None),
            (
                Code::BadFormat,
                Source::System,
                "80-bad.toml",
                Some(1),
                Some("rate")
            ),
            (Code::Syntax, Source::System, "85-syntax.toml", None, None),
            (Code::NotUtf8, Source::System, "95-gbk.toml", None, None),
        ],
        "照文件名排"
    );
    let bad = &loaded.problems[1];
    assert_eq!(bad.at.map(|at| at.line), Some(3));
    assert_eq!(bad.got.as_deref(), Some("\"abc\""));
    clean(&dir);
}

#[test]
fn a_system_file_that_cannot_be_read_still_replaces_the_same_name() {
    let (dir, root) = temp_root();
    system_rule(
        &root,
        "50-defaults.toml",
        b"[[rule]]\nparallel = 2 # \xff\n",
    );
    let loaded = load(&factory(), &root);
    assert_eq!(
        loaded.at(&group("1")).resolved.entries.len(),
        0,
        "读不成的照空的用：出厂同名的那份也不用，和写法不对一样"
    );
    assert_eq!(loaded.problems.len(), 1);
    assert_eq!(loaded.problems[0].code, Code::NotUtf8);
    assert_eq!(loaded.problems[0].file, "50-defaults.toml");
    clean(&dir);
}

#[test]
fn venues_d_that_cannot_be_listed_is_a_problem() {
    let (dir, root) = temp_root();
    std::fs::write(root.system().join("venues.d"), "not a directory").expect("写得进");
    let loaded = load(&factory(), &root);
    assert_eq!(loaded.problems.len(), 1);
    let problem = &loaded.problems[0];
    assert_eq!(
        (problem.code, problem.source, problem.file.as_str()),
        (Code::Unreadable, Source::System, "venues.d")
    );
    assert!(problem.why.is_some(), "带系统的原话");
    assert_eq!(
        loaded.at(&group("1")).resolved.entries["parallel"].value,
        Value::Int(1),
        "出厂的照用"
    );
    clean(&dir);
}

#[test]
fn the_system_word_list_replaces_the_factory_one() {
    let (dir, root) = temp_root();
    let factory = factory();
    let path = system_words(&root, "# 我们群的\nfoo\n\n  bar  \nfoo\n");
    let loaded = load(&factory, &root);
    assert_eq!(
        loaded.keywords,
        ["foo", "bar"],
        "整份替换，照群聊内核的读法"
    );
    assert_eq!(loaded.problems, []);
    std::fs::write(&path, b"foo\n\xff\n").expect("写得进");
    let loaded = load(&factory, &root);
    assert_eq!(loaded.keywords, Vec::<String>::new(), "读不成的照空的用");
    assert_eq!(loaded.problems.len(), 1);
    let problem = &loaded.problems[0];
    assert_eq!(
        (problem.code, problem.source, problem.file.as_str()),
        (Code::NotUtf8, Source::System, "moderation.txt")
    );
    std::fs::remove_file(&path).expect("删得了");
    assert_eq!(load(&factory, &root).keywords.len(), FACTORY_WORDS);
    clean(&dir);
}

#[test]
fn word_list_problems_come_after_the_rule_files() {
    let (dir, root) = temp_root();
    system_words(&root, b"\xff");
    system_rule(&root, "99-bad.toml", "nonsense = 1\n");
    let files: Vec<String> = load(&factory(), &root)
        .problems
        .into_iter()
        .map(|problem| problem.file)
        .collect();
    assert_eq!(files, ["99-bad.toml", "moderation.txt"]);
    clean(&dir);
}

#[test]
fn factory_mistakes_are_all_reported() {
    let resources = copied_resources();
    let factory = resources.join("software/onebot");
    std::fs::write(
        factory.join("venues.d/50-defaults.toml"),
        "[[rule]]\nrate = \"abc\"\n",
    )
    .expect("写得进");
    let defaults = std::fs::read_to_string(factory.join("defaults.toml")).expect("读得了");
    std::fs::write(
        factory.join("defaults.toml"),
        defaults.replace("probability = 50 ", "probability = 5000 "),
    )
    .expect("写得进");
    std::fs::remove_file(factory.join("moderation.txt")).expect("删得了");
    let problems = Factory::load(&ResourceRoot::at(&resources)).expect_err("读不出来");
    let seen = problems
        .iter()
        .map(|problem| {
            (
                problem.code,
                problem.source,
                problem.file.as_str(),
                problem.key.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        seen,
        [
            (
                Code::BadFormat,
                Source::Factory,
                "50-defaults.toml",
                Some("rate")
            ),
            (
                Code::OutOfRange,
                Source::Factory,
                "defaults.toml",
                Some("chatty.probability")
            ),
            (Code::Unreadable, Source::Factory, "moderation.txt", None),
        ]
    );
    clean(resources.parent().expect("有上一级"));
}

#[test]
fn broken_judge_texts_are_factory_mistakes() {
    // 判官的说明（施工 O-23 下）：`violations.txt` 的模板写坏了、要了别的字段，记一条 `bad_format`；少了一份记一条读不成。
    let resources = copied_resources();
    let judge = resources.join("software/onebot/judge");
    std::fs::write(judge.join("violations.txt"), "Severity {nope}.\n").expect("写得进");
    let problems = Factory::load(&ResourceRoot::at(&resources)).expect_err("模板写坏了");
    assert_eq!(
        problems
            .iter()
            .map(|problem| (problem.code, problem.source, problem.file.as_str()))
            .collect::<Vec<_>>(),
        [(Code::BadFormat, Source::Factory, "judge/violations.txt")]
    );
    assert!(
        problems[0]
            .why
            .as_deref()
            .is_some_and(|why| why.contains("nope")),
        "{problems:?}"
    );
    std::fs::copy(
        resources_file("software/onebot/judge/violations.txt"),
        judge.join("violations.txt"),
    )
    .expect("抄得了");
    std::fs::remove_file(judge.join("answer.txt")).expect("删得了");
    let problems = Factory::load(&ResourceRoot::at(&resources)).expect_err("少了一份");
    assert_eq!(
        problems
            .iter()
            .map(|problem| (problem.code, problem.file.as_str()))
            .collect::<Vec<_>>(),
        [(Code::Unreadable, "judge/answer.txt")]
    );
    clean(resources.parent().expect("有上一级"));
}

/// 源码树里资源目录的一份文件。
fn resources_file(path: &str) -> PathBuf {
    resources().join(path)
}

#[test]
fn a_factory_warning_or_a_missing_directory_is_a_mistake_too() {
    let resources = copied_resources();
    let rules = resources.join("software/onebot/venues.d");
    std::fs::write(rules.join("60-extra.toml"), "nonsense = 1\n").expect("写得进");
    let problems = Factory::load(&ResourceRoot::at(&resources)).expect_err("警告也算");
    assert_eq!(problems.len(), 1);
    assert_eq!(
        (problems[0].code, problems[0].file.as_str()),
        (Code::UnknownKey, "60-extra.toml")
    );
    std::fs::remove_dir_all(&rules).expect("删得了");
    let problems = Factory::load(&ResourceRoot::at(&resources)).expect_err("出厂的规则不在");
    assert_eq!(
        problems
            .iter()
            .map(|problem| (problem.code, problem.file.as_str()))
            .collect::<Vec<_>>(),
        [(Code::Unreadable, "venues.d")]
    );
    clean(resources.parent().expect("有上一级"));
}
