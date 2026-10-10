//! 报错的话（施工 8-2，`docs/blueprint/config.md`「样子」）：照源码树的资源，登记的全部清单，每一种原因码说成一句；
//! 中文、英文和图纸里的例子一字不差，日文每一句都说得出来。

use std::path::{Path, PathBuf};

use miyu_config::merge::Origin;
use miyu_config::problem::{At, Code, Problem, Using, tell};
use miyu_config::{Layer, Value};
use miyu_core::settings::items;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

fn words(language: &str) -> Human {
    let repository: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Human::load(&ResourceRoot::at(repository.join("resources")), language)
        .unwrap_or_else(|error| panic!("{language} 的字读得出来：{error}"))
}

fn at(line: usize, column: usize) -> At {
    At { line, column }
}

/// 图纸「样子」里的几种，和现在照什么用着。
fn samples() -> Vec<(Problem, Option<Using>)> {
    let default = |value: Value| Some(Using::Value(value, Origin::Default));
    let text = |text: &'static str| Value::Text(std::borrow::Cow::Borrowed(text));
    let mut unknown = Problem::item(
        Code::UnknownKey,
        Layer::Personal,
        "ui.langauge",
        at(7, 1),
        "\"zh\"",
    );
    unknown.suggest = Some("ui.language");
    let mut looser = Problem::item(
        Code::NotTightening,
        Layer::Project,
        "permission.start_read_only",
        at(3, 19),
        "false",
    );
    looser.current = Some(Value::Bool(true));
    let syntax = Problem {
        at: Some(at(4, 12)),
        ..Problem::file(
            Code::Syntax,
            Layer::Personal,
            Some("invalid basic string".to_string()),
        )
    };
    vec![
        (unknown, None),
        (
            Problem::item(
                Code::NotAnOption,
                Layer::System,
                "log.level",
                at(2, 9),
                "\"verbose\"",
            ),
            default(text("info")),
        ),
        (looser, None),
        (syntax, Some(Using::LastGood)),
        (
            Problem::item(
                Code::WrongLayer,
                Layer::Personal,
                "log.level",
                at(9, 9),
                "\"debug\"",
            ),
            default(text("info")),
        ),
        (
            Problem::item(
                Code::WrongType,
                Layer::Personal,
                "permission.start_read_only",
                at(5, 19),
                "\"yes\"",
            ),
            default(Value::Bool(false)),
        ),
    ]
}

fn told(language: &str) -> Vec<String> {
    let words = words(language);
    samples()
        .iter()
        .map(|(problem, using)| {
            tell(problem, &items(), using.as_ref(), &words)
                .unwrap_or_else(|missing| panic!("{language}：{missing}"))
                .message
        })
        .collect()
}

#[test]
fn chinese_says_what_the_blueprint_shows() {
    assert_eq!(
        told("zh"),
        [
            "没有 ui.langauge 这一项。是不是想写 ui.language？已忽略此行。",
            "log.level 只能是 error、warn、info、debug、trace 或 off，写的是 \"verbose\"。改成其中一个，例如 log.level = \"info\"。暂用 \"info\"（默认值）。",
            "项目配置只能让限制更严。permission.start_read_only 现在是 true，这里写的 false 更宽，不算。",
            "TOML 写法不对：invalid basic string。暂用上次的有效配置。",
            "log.level 只能写在系统配置里，写在个人设置里不算。挪到系统配置里去。",
            "permission.start_read_only 要写 true 或 false，写的是 \"yes\"。改成 permission.start_read_only = true。暂用 false（默认值）。",
        ]
    );
}

#[test]
fn english_says_what_the_blueprint_shows() {
    assert_eq!(
        told("en"),
        [
            "There is no ui.langauge. Did you mean ui.language? Line ignored.",
            "log.level must be error, warn, info, debug, trace or off, not \"verbose\". Write one of them, e.g. log.level = \"info\". Using \"info\" (the default) for now.",
            "A project config can only make limits stricter. permission.start_read_only is true, and false here is looser, so it does not count.",
            "Not valid TOML: invalid basic string. Using the last valid config.",
            "log.level belongs in the system config. It does not count in personal settings. Move it to the system config.",
            "permission.start_read_only needs true or false, not \"yes\". Write permission.start_read_only = true. Using false (the default) for now.",
        ]
    );
}

#[test]
fn every_code_is_said_in_every_language() {
    let mut all = samples();
    for code in [
        Code::Unreadable,
        Code::TooBig,
        Code::NotUtf8,
        Code::UntrustedProject,
    ] {
        all.push((
            Problem::file(code, Layer::System, Some("why".to_string())),
            Some(Using::Nothing),
        ));
    }
    all.push((
        Problem::item(Code::WrongType, Layer::Personal, "ui", at(1, 1), "1"),
        None,
    ));
    all.push((
        Problem::item(Code::UnknownKey, Layer::Personal, "zzz.zzz", at(1, 1), "1"),
        None,
    ));
    // 施工 8-5：引用取不到的两种，密钥文件里写错的两种。
    for code in [Code::UnknownSecret, Code::EnvNotSet] {
        let mut problem = Problem::item(code, Layer::System, "providers.demo", at(1, 1), "{}");
        problem.name = Some("deepseek".to_string());
        all.push((problem, None));
    }
    for code in [Code::SecretName, Code::SecretValue] {
        all.push((
            Problem {
                key: Some("DeepSeek".to_string()),
                ..Problem::file(code, Layer::System, None)
            },
            None,
        ));
    }
    for language in ["zh", "en", "ja"] {
        let words = words(language);
        assert!(
            miyu_config::Words::sentence(&words, "config/secrets-header", &[]).is_some(),
            "{language} 密钥文件开头的注释"
        );
        for (problem, using) in &all {
            let told = tell(problem, &items(), using.as_ref(), &words)
                .unwrap_or_else(|missing| panic!("{language} {:?}：{missing}", problem.code));
            assert!(!told.message.is_empty(), "{language} {:?}", problem.code);
        }
    }
}
