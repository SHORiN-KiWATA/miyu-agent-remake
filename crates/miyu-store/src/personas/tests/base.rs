//! 以谁为底（施工 P-3 上，16 第四节）：`persona.toml` 逐项盖在底叠好的样子上，提示词自己的几层没有的沿用底的，来自哪儿写明
//! 是底的哪一层；底也照这样找；绕成圈、指着没有的、底写错了的报错。

use super::*;

#[test]
fn own_layers_go_over_the_base_and_prompts_fall_back_to_it() {
    let places = Places::new();
    places.write(
        Layer::Shipped,
        "engineer",
        "persona.toml",
        "[persona]\nname = { en = \"Engineer\", zh = \"工程师\" }\n\n[memory]\nscope = \"session\"\n",
    );
    places.write(
        Layer::Shipped,
        "engineer",
        "prompts/persona.md",
        "be precise\n",
    );
    places.write(
        Layer::System,
        "engineer",
        "prompts/examples.md",
        "user: a\nassistant: b\n",
    );
    places.write(
        Layer::Home,
        "mine",
        "persona.toml",
        "[persona]\nbase = \"engineer\"\nname = { zh = \"我的\" }\n",
    );
    places.write(Layer::Home, "mine", "prompts/reminders.md", "Stay short.\n");
    let found = places.personas.find("mine").unwrap();
    assert_eq!(found.base.as_deref(), Some("engineer"));
    assert_eq!(found.layers, [Layer::Home]);
    assert_eq!(
        (
            found.file.name.get("zh").map(String::as_str),
            found.file.name.get("en").map(String::as_str)
        ),
        (Some("我的"), Some("Engineer"))
    );
    assert_eq!(
        found.file.memory,
        Some(miyu_policy::memory::MemoryScope::Session)
    );
    assert_eq!(
        found.texts.persona, "be precise\n",
        "自己没有的人设沿用底的"
    );
    let origin = |persona: &str, layer: Layer| {
        Some(Origin {
            persona: persona.to_string(),
            layer,
        })
    };
    assert_eq!(found.persona_from, origin("engineer", Layer::Shipped));
    assert_eq!(found.examples_from, origin("engineer", Layer::System));
    assert_eq!(found.texts.reminders, "Stay short.\n");
    assert_eq!(found.reminders_from, origin("mine", Layer::Home));
    assert_eq!(
        found.home,
        Some(AccountId::parse("admin").unwrap()),
        "住在自己的家目录：记忆照旧归自己"
    );
}

#[test]
fn cycles_missing_bases_and_broken_bases_are_errors() {
    let places = Places::new();
    places.write(
        Layer::Home,
        "a",
        "persona.toml",
        "[persona]\nbase = \"b\"\n",
    );
    places.write(
        Layer::Home,
        "b",
        "persona.toml",
        "[persona]\nbase = \"c\"\n",
    );
    places.write(
        Layer::Home,
        "c",
        "persona.toml",
        "[persona]\nbase = \"a\"\n",
    );
    places.write(
        Layer::Home,
        "lost",
        "persona.toml",
        "[persona]\nbase = \"nowhere\"\n",
    );
    places.write(
        Layer::Home,
        "on-broken",
        "persona.toml",
        "[persona]\nbase = \"broken\"\n",
    );
    places.write(
        Layer::Shipped,
        "broken",
        "prompts/examples.md",
        "assistant: hi\n",
    );
    places.write(Layer::Home, "bad", "persona.toml", "[persona]\nbase = 3\n");
    let error = |id: &str| places.personas.find(id).unwrap_err().to_string();
    assert_eq!(error("a"), "base cycle: a -> b -> c -> a");
    assert_eq!(error("lost"), r#"base "nowhere" of "lost" not found"#);
    assert!(
        error("on-broken").starts_with("base broken: shipped prompts/examples.md:1: "),
        "底写错了说是哪个底：{}",
        error("on-broken")
    );
    let PersonaError::Invalid(_, problem) = places.personas.find("bad").unwrap_err() else {
        panic!("写错了的底是文件写错");
    };
    assert_eq!(problem.code, miyu_policy::persona::Code::BadBase);
}
