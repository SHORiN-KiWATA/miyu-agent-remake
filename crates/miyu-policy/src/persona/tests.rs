use super::*;

fn phrases(pairs: &[(&str, &str)]) -> Option<Label> {
    Some(Label::Each(
        pairs
            .iter()
            .map(|(language, text)| (language.to_string(), text.to_string()))
            .collect(),
    ))
}

fn one(text: &str) -> Option<Label> {
    Some(Label::One(text.to_string()))
}

#[test]
fn names_are_one_line_and_old_language_tables_still_read() {
    let file = read_toml("[persona]\nname = \"  我的工程师 \"\nsummary = \"写代码\"\n").unwrap();
    assert_eq!(
        (file.name, file.summary),
        (one("我的工程师"), one("写代码"))
    );
    let file = read_toml(
        "[persona]\nname = { en = \"Software Engineer\", zh = \"软件工程师\" }\n\n[persona.summary]\nja = \"  エンジニア \"\n",
    )
    .unwrap();
    assert_eq!(
        file.name,
        phrases(&[("en", "Software Engineer"), ("zh", "软件工程师")])
    );
    assert_eq!(
        file.summary,
        phrases(&[("ja", "エンジニア")]),
        "去掉前后空白"
    );
    let empty = read_toml("[persona]\nname = \"  \"\n").unwrap_err();
    assert_eq!(
        (empty.code, empty.detail.as_str()),
        (Code::EmptyPhrase, "persona.name")
    );
    let number = read_toml("[persona]\nname = 3\n").unwrap_err();
    assert_eq!(number.code, Code::NotPhrases);
    assert_eq!(read_toml("").unwrap(), PersonaFile::default(), "空的文件");
    assert_eq!(read_toml("[persona]\n").unwrap(), PersonaFile::default());
}

#[test]
fn a_wrong_toml_says_which_line() {
    for (text, line, message) in [
        ("[persona]\nname = 1\n", 2, "persona.name must be text"),
        ("[knowledge]\nbases = []\n", 1, "unknown table [knowledge]"),
        (
            "[memory]\nscope = \"off\"\n",
            2,
            "memory.scope must be persona or session",
        ),
        ("[memory]\nkeep = 1\n", 2, "unknown key memory.keep"),
        ("memory = 3\n", 1, "memory must be a table"),
        (
            "[persona]\n\nvoice = \"x\"\n",
            3,
            "unknown key persona.voice",
        ),
        (
            "[persona]\nname = { fr = \"Ingénieur\" }\n",
            2,
            "persona.name.fr: language must be zh, en or ja",
        ),
        (
            "[persona]\nsummary = { en = \" \" }\n",
            2,
            "persona.summary.en must be non-empty text",
        ),
        ("persona = 3\n", 1, "persona must be a table"),
    ] {
        let problem = read_toml(text).unwrap_err();
        assert_eq!(problem.file, "persona.toml");
        assert_eq!(
            (problem.line, problem.message.as_str()),
            (Some(line), message),
            "{text:?}"
        );
    }
    let broken = read_toml("[persona\n").unwrap_err();
    assert_eq!(broken.line, Some(1), "读不成 TOML 的也说第几行");
    assert!(!broken.message.is_empty());
}

#[test]
fn an_upper_layer_replaces_what_it_writes() {
    let shipped = PersonaFile {
        name: phrases(&[("en", "Engineer"), ("zh", "工程师")]),
        summary: phrases(&[("en", "Helps.")]),
        memory: None,
    };
    let mine = PersonaFile {
        name: one("我的工程师"),
        summary: None,
        memory: None,
    };
    let merged = mine.over(shipped);
    assert_eq!(merged.name, one("我的工程师"), "写了的整格换掉");
    assert_eq!(merged.summary, phrases(&[("en", "Helps.")]), "没写的沿用");
}

#[test]
fn examples_are_read_like_the_old_dialogs_file() {
    let text = "User: 问个事，这个怎么装？\nassistant: AUR 上有\n\nuser: 那 192 乘以 45 呢？\nassistant:8640\n第二行接着说\n\n\nUSER:  两个空格\nAssistant: 好\n";
    let demos = read_examples(text).unwrap();
    assert_eq!(
        demos,
        [
            Demo {
                user: "问个事，这个怎么装？".into(),
                assistant: "AUR 上有".into(),
            },
            Demo {
                user: "那 192 乘以 45 呢？".into(),
                assistant: "8640\n第二行接着说".into(),
            },
            Demo {
                user: "两个空格".into(),
                assistant: "好".into(),
            },
        ]
    );
    assert!(read_examples("").unwrap().is_empty());
    assert!(read_examples("\n\n").unwrap().is_empty());
}

#[test]
fn wrong_examples_say_which_line() {
    for (text, line, message) in [
        (
            "hello\nuser: hi\n",
            1,
            "the first line must start with user:",
        ),
        ("assistant: hi\n", 1, "the first line must start with user:"),
        (
            "user: a\nuser: b\n",
            2,
            "user and assistant must take turns",
        ),
        (
            "user: a\nassistant: b\nassistant: c\n",
            3,
            "user and assistant must take turns",
        ),
        (
            "user: a\nassistant: b\n\nuser: c\n",
            4,
            "the last line must be the assistant's",
        ),
        ("user: \nassistant: b\n", 1, "a line must say something"),
        ("user: a\nassistant:   \n", 2, "a line must say something"),
    ] {
        let problem = read_examples(text).unwrap_err();
        assert_eq!(problem.file, "prompts/examples.md");
        assert_eq!(
            (problem.line, problem.message.as_str()),
            (Some(line), message),
            "{text:?}"
        );
    }
}

#[test]
fn a_problem_reads_as_file_line_and_message() {
    let problem = read_examples("user: a\n").unwrap_err();
    assert_eq!(
        problem.to_string(),
        "prompts/examples.md:1: the last line must be the assistant's"
    );
}

/// 示范对话进快照（施工 P-1 上）：组装时排在 system 后面、历史前面，一轮两条，`stable` 数的是它们；没有示范对话的人格，
/// 快照的字节里没有这一格，和以前一样。
#[test]
fn examples_go_after_the_system_and_before_the_history() {
    use crate::compose::{PersonaTexts, Sources, compose};
    use miyu_kernel::block::Block;
    use miyu_kernel::history::History;
    use miyu_kernel::request::Message;

    let examples =
        read_examples("user: 在吗\nassistant: 在\nuser: 几点了\nassistant: 九点\n").unwrap();
    let sources = Sources {
        core: crate::test_support::core(),
        persona: PersonaTexts {
            persona: "You are Miyu.\n".to_string(),
            examples,
            reminders: String::new(),
        },
        reminder: Default::default(),
    };
    let snapshot = compose("miyu", sources, true);
    let request = snapshot
        .policy()
        .unwrap()
        .assembler
        .assemble(&History::default());
    assert_eq!(request.system, "You are Miyu.");
    assert_eq!(request.stable, 4);
    let said: Vec<(&str, String)> = request
        .messages
        .iter()
        .map(|message| match message {
            Message::User { blocks } => ("user", text(blocks)),
            Message::Assistant { blocks } => ("assistant", text(blocks)),
            other => ("other", format!("{other:?}")),
        })
        .collect();
    assert_eq!(
        said,
        [
            ("user", "在吗".to_string()),
            ("assistant", "在".to_string()),
            ("user", "几点了".to_string()),
            ("assistant", "九点".to_string()),
        ]
    );
    let bytes = String::from_utf8(snapshot.to_bytes()).unwrap();
    assert!(
        bytes.contains(r#""demos":[{"user":"在吗","assistant":"在"}"#),
        "{bytes}"
    );
    let engineer = String::from_utf8(crate::test_support::engineer().to_bytes()).unwrap();
    assert!(!engineer.contains("demos"), "没有示范对话的不写这一格");

    fn text(blocks: &[Block]) -> String {
        blocks
            .iter()
            .map(|block| match block {
                Block::Text(text) => text.text.clone(),
                other => format!("{other:?}"),
            })
            .collect()
    }
}

#[test]
fn the_memory_scope_is_persona_or_session_and_an_upper_layer_wins() {
    let read = |text: &str| read_toml(text).unwrap().memory;
    assert_eq!(
        read("[memory]\nscope = \"persona\"\n"),
        Some(MemoryScope::Persona)
    );
    assert_eq!(
        read("[memory]\nscope = \"session\"\n"),
        Some(MemoryScope::Session)
    );
    assert_eq!(read("[memory]\n"), None, "不写是没有，照 persona 算");
    let lower = read_toml("[memory]\nscope = \"session\"\n").unwrap();
    assert_eq!(
        PersonaFile::default().over(lower.clone()).memory,
        Some(MemoryScope::Session),
        "上一层没写沿用下面的"
    );
    let upper = read_toml("[memory]\nscope = \"persona\"\n").unwrap();
    assert_eq!(upper.over(lower).memory, Some(MemoryScope::Persona));
}

/// 每一种错有自己的代码，错的那一处另放一格：给人看的那一句照它们写（施工 8-30）。
#[test]
fn each_mistake_carries_its_code_and_where() {
    for (text, code, detail) in [
        ("[persona\n", Code::Syntax, None),
        ("[voice]\n", Code::UnknownTable, Some("voice")),
        ("memory = 1\n", Code::NotATable, Some("memory")),
        (
            "[memory]\nkind = 1\n",
            Code::UnknownKey,
            Some("memory.kind"),
        ),
        (
            "[memory]\nscope = \"off\"\n",
            Code::BadMemoryScope,
            Some("memory.scope"),
        ),
        ("persona = 3\n", Code::NotATable, Some("persona")),
        (
            "[persona]\nvoice = 1\n",
            Code::UnknownKey,
            Some("persona.voice"),
        ),
        (
            "[persona]\nname = 1\n",
            Code::NotPhrases,
            Some("persona.name"),
        ),
        (
            "[persona]\nname = { fr = \"x\" }\n",
            Code::UnknownLanguage,
            Some("persona.name.fr"),
        ),
        (
            "[persona]\nsummary = { en = \"\" }\n",
            Code::EmptyPhrase,
            Some("persona.summary.en"),
        ),
    ] {
        let problem = read_toml(text).unwrap_err();
        assert_eq!(problem.code, code, "{text:?}");
        if let Some(detail) = detail {
            assert_eq!(problem.detail, detail, "{text:?}");
        } else {
            assert!(!problem.detail.is_empty(), "读不成 TOML 的带它的原话");
        }
    }
    for (text, code) in [
        ("assistant: a\n", Code::FirstLine),
        ("user: a\nuser: b\n", Code::TakeTurns),
        ("user: a\n", Code::LastLine),
        ("user: \nassistant: b\n", Code::EmptyLine),
    ] {
        let problem = read_examples(text).unwrap_err();
        assert_eq!(
            (problem.code, problem.detail.as_str()),
            (code, ""),
            "{text:?}"
        );
    }
    let names: Vec<&str> = Code::ALL.iter().map(|code| code.as_str()).collect();
    let mut unique = names.clone();
    unique.dedup();
    assert_eq!(names.len(), unique.len(), "写法不重复");
}

#[test]
fn examples_are_written_back_and_read_the_same() {
    let demos = vec![
        Demo {
            user: "  问个事 ".to_string(),
            assistant: "第一行\n\n第二行".to_string(),
        },
        Demo {
            user: "user 说的".to_string(),
            assistant: "好".to_string(),
        },
    ];
    let text = write_examples(&demos).unwrap();
    assert_eq!(
        text,
        "user: 问个事\nassistant: 第一行\n第二行\n\nuser: user 说的\nassistant: 好\n"
    );
    assert_eq!(
        read_examples(&text).unwrap(),
        [
            Demo {
                user: "问个事".to_string(),
                assistant: "第一行\n第二行".to_string()
            },
            Demo {
                user: "user 说的".to_string(),
                assistant: "好".to_string()
            },
        ],
        "空行去掉，读回来一样"
    );
    let clash = vec![
        Demo {
            user: "a".to_string(),
            assistant: "b".to_string(),
        },
        Demo {
            user: "c".to_string(),
            assistant: "照这样写：\nuser: 你好".to_string(),
        },
    ];
    assert_eq!(write_examples(&clash), Err(2), "第二对有一行像 user: 开头");
    assert_eq!(write_examples(&[]), Ok(String::new()));
}
