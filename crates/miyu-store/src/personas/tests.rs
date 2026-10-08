//! 人格的三层怎么叠（施工 P-1 上）。

use std::fs;

use super::*;
use crate::env::{Env, Platform};
use crate::test_support::Scratch;

/// 一个临时的资源目录和数据根：`res/`、`home/`。
struct Places {
    scratch: Scratch,
    personas: Personas,
}

impl Places {
    fn new() -> Places {
        let scratch = Scratch::new();
        let resources = ResourceRoot::at(scratch.path().join("res"));
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(scratch.path().join("data").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        let root = DataRoot::locate(&env).unwrap();
        let admin = AccountId::parse("admin").unwrap();
        let personas = Personas::new(&resources, &root, &admin);
        Places { scratch, personas }
    }

    /// 在一层里写人格 `id` 的一个文件。
    fn write(&self, layer: Layer, id: &str, file: &str, text: &str) {
        let base = match layer {
            Layer::Shipped => "res/personas",
            Layer::System => "data/system/personas",
            Layer::Home => "data/home/admin/personas",
        };
        let path = self.scratch.path().join(base).join(id).join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

#[test]
fn the_layers_stack_file_by_file_and_key_by_key() {
    let places = Places::new();
    places.write(
        Layer::Shipped,
        "miyu",
        "persona.toml",
        "[persona]\nname = { en = \"Miyu\", zh = \"美羽\" }\nsummary = { en = \"Shipped.\" }\n",
    );
    places.write(
        Layer::Shipped,
        "miyu",
        "prompts/persona.md",
        "shipped persona\n",
    );
    places.write(
        Layer::Shipped,
        "miyu",
        "prompts/examples.md",
        "user: a\nassistant: b\n",
    );
    places.write(
        Layer::System,
        "miyu",
        "persona.toml",
        "[persona]\nsummary = { en = \"System.\" }\n",
    );
    places.write(Layer::Home, "miyu", "prompts/persona.md", "my persona\n");
    places.write(
        Layer::System,
        "miyu",
        "prompts/reminders.md",
        "Stay soft.\n",
    );
    let found = places.personas.find("miyu").unwrap();
    assert_eq!(found.layers, [Layer::Shipped, Layer::System, Layer::Home]);
    assert_eq!(
        found.file.name.as_ref().and_then(|name| name.pick("zh")),
        Some("美羽"),
        "没盖的沿用"
    );
    assert_eq!(
        found
            .file
            .summary
            .as_ref()
            .and_then(|summary| summary.pick("en")),
        Some("System.")
    );
    assert_eq!(found.texts.persona, "my persona\n", "人设同名替换，原样");
    assert_eq!(found.persona_from, Some(Layer::Home));
    assert_eq!(found.texts.examples.len(), 1, "示范对话沿用出厂的");
    assert_eq!(found.examples_from, Some(Layer::Shipped));
    assert_eq!(
        found.texts.reminders, "Stay soft.\n",
        "角色扮演提示照层叠，原样"
    );
    assert_eq!(found.reminders_from, Some(Layer::System));
    assert_eq!(found.home, Some(AccountId::parse("admin").unwrap()));
}

#[test]
fn a_persona_only_in_the_shipped_layer_lives_in_no_home() {
    let places = Places::new();
    places.write(Layer::Shipped, "engineer", "prompts/persona.md", "x\n");
    let found = places.personas.find("engineer").unwrap();
    assert_eq!(found.home, None);
    assert_eq!(found.file, PersonaFile::default(), "没有 persona.toml 也行");
    assert!(found.texts.examples.is_empty());
    assert_eq!(
        (found.texts.reminders.as_str(), found.reminders_from),
        ("", None)
    );
    places.write(Layer::System, "shared", "prompts/persona.md", "y\n");
    assert_eq!(
        places.personas.find("shared").unwrap().home,
        None,
        "系统区的也不住在谁家"
    );
    // 家目录里只有一个空目录，也算住在那里。
    fs::create_dir_all(places.scratch.path().join("data/home/admin/personas/bare")).unwrap();
    let bare = places.personas.find("bare").unwrap();
    assert_eq!(bare.home, Some(AccountId::parse("admin").unwrap()));
    assert_eq!(bare.texts.persona, "", "没有人设是空的");
}

#[test]
fn missing_and_badly_named_personas_are_refused() {
    let places = Places::new();
    assert!(matches!(
        places.personas.find("nobody"),
        Err(PersonaError::NotFound(_))
    ));
    for id in ["", "Miyu", "../x", "a/b", "1abc", &"a".repeat(65)] {
        assert!(
            matches!(places.personas.find(id), Err(PersonaError::BadId(_))),
            "{id:?}"
        );
    }
    // 是文件、不是目录的不算。
    fs::create_dir_all(places.scratch.path().join("res/personas")).unwrap();
    fs::write(places.scratch.path().join("res/personas/file"), "x").unwrap();
    assert!(matches!(
        places.personas.find("file"),
        Err(PersonaError::NotFound(_))
    ));
}

#[test]
fn a_wrong_file_says_which_layer_and_line() {
    let places = Places::new();
    places.write(
        Layer::Shipped,
        "miyu",
        "prompts/examples.md",
        "user: a\nassistant: b\n",
    );
    places.write(
        Layer::Home,
        "miyu",
        "prompts/examples.md",
        "user: a\nuser: b\n",
    );
    let error = places.personas.find("miyu").unwrap_err();
    assert_eq!(
        error.to_string(),
        "home prompts/examples.md:2: user and assistant must take turns"
    );
    places.write(Layer::System, "other", "persona.toml", "[voice]\n");
    let error = places.personas.find("other").unwrap_err();
    assert_eq!(
        error.to_string(),
        "system persona.toml:1: unknown table [voice]"
    );
}

#[test]
fn ids_come_from_every_layer_once_in_order() {
    let places = Places::new();
    assert!(places.personas.ids().is_empty(), "几层都还没有");
    places.write(Layer::Shipped, "engineer", "prompts/persona.md", "x\n");
    places.write(Layer::Home, "miyu", "prompts/persona.md", "y\n");
    places.write(Layer::Home, "engineer", "persona.toml", "");
    places.write(Layer::System, "Bad Name", "persona.toml", "");
    fs::write(
        places.scratch.path().join("data/home/admin/personas/loose"),
        "x",
    )
    .unwrap();
    assert_eq!(places.personas.ids(), ["engineer", "miyu"]);
}

/// 记忆归哪个账号（施工 P-1 上，`personas.md`「怎么走」第 5 条）：住在家目录里的归那个账号，出厂、系统区的归会话的属主；
/// 只看目录，文件写错了也照算。
#[test]
fn memory_goes_to_the_home_the_persona_lives_in() {
    let places = Places::new();
    let owner = AccountId::parse("bob").unwrap();
    places.write(Layer::Shipped, "engineer", "prompts/persona.md", "x\n");
    places.write(Layer::System, "shared", "prompts/persona.md", "y\n");
    places.write(
        Layer::Home,
        "miyu",
        "prompts/examples.md",
        "user: a\nuser: b\n",
    );
    let personas = &places.personas;
    assert_eq!(personas.memory_account("engineer", &owner), owner);
    assert_eq!(personas.memory_account("shared", &owner), owner);
    assert_eq!(personas.memory_account("miyu", &owner).as_str(), "admin");
    assert_eq!(
        personas.memory_account("gone", &owner),
        owner,
        "删掉了的归属主"
    );
    assert_eq!(
        personas.memory_account("../admin", &owner),
        owner,
        "不合写法的不找"
    );
    assert_eq!(
        personas
            .home_of("miyu")
            .map(|account| account.as_str().to_string()),
        Some("admin".to_string())
    );
    assert_eq!(personas.home_of("engineer"), None);
}

/// `miyu check` 用的查法（施工 8-30）：每一层各查各的，上面一层盖住了照样报；只查一份文件的照它在哪认。
#[test]
fn every_layer_is_checked_on_its_own() {
    let places = Places::new();
    places.write(Layer::Shipped, "miyu", "persona.toml", "[voice]\n");
    places.write(Layer::Home, "miyu", "persona.toml", "[persona]\n");
    places.write(Layer::Home, "miyu", "prompts/examples.md", "user: a\n");
    places.write(Layer::System, "fine", "prompts/persona.md", "x\n");
    let found = places.personas.check();
    let seen: Vec<(Layer, String, &'static str)> = found
        .iter()
        .map(|checked| {
            // 照路径的各段比，三个平台一样。
            let file = checked
                .path
                .strip_prefix(places.scratch.path())
                .unwrap()
                .iter()
                .map(|part| part.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let code = match &checked.issue {
                Issue::Wrong(problem) => problem.code.as_str(),
                Issue::Unreadable(_) => "unreadable",
            };
            (checked.layer, file, code)
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                Layer::Shipped,
                "res/personas/miyu/persona.toml".to_string(),
                "unknown_table"
            ),
            (
                Layer::Home,
                "data/home/admin/personas/miyu/prompts/examples.md".to_string(),
                "last_line"
            ),
        ],
        "出厂的被家目录的盖住了也报"
    );
    let one = places
        .scratch
        .path()
        .join("data/home/admin/personas/miyu/prompts/examples.md");
    let checked = places.personas.check_file(&one).expect("是人格的文件");
    assert_eq!(checked.len(), 1);
    let fine = places
        .scratch
        .path()
        .join("data/home/admin/personas/miyu/persona.toml");
    assert!(
        places
            .personas
            .check_file(&fine)
            .expect("是人格的文件")
            .is_empty()
    );
    let missing = places
        .scratch
        .path()
        .join("data/system/personas/fine/persona.toml");
    let checked = places.personas.check_file(&missing).expect("是人格的文件");
    assert!(
        matches!(checked[0].issue, Issue::Unreadable(_)),
        "写了文件的，没有就是读不了"
    );
    // 经链接传的路径照样认得（macOS 的 `/var` 是链接，CI 撞见过）。
    #[cfg(unix)]
    {
        let link = places.scratch.path().join("link");
        std::os::unix::fs::symlink(places.scratch.path().join("data"), &link).unwrap();
        let through = link.join("home/admin/personas/miyu/prompts/examples.md");
        let checked = places
            .personas
            .check_file(&through)
            .expect("经链接也是人格的文件");
        assert_eq!(checked.len(), 1);
        let fresh = link.join("home/admin/personas/fresh/persona.toml");
        assert!(
            places.personas.check_file(&fresh).is_some(),
            "还没建的人格目录也认"
        );
    }
    let other = places
        .scratch
        .path()
        .join("data/home/admin/personas/miyu/prompts/persona.md");
    assert!(places.personas.check_file(&other).is_none(), "人设不查");
    assert!(
        places
            .personas
            .check_file(&places.scratch.path().join("elsewhere.toml"))
            .is_none()
    );
}
