//! 找资源目录的几种情形，和读一个人格的原文。

use std::fs;

use super::*;
use crate::env::Platform;
use crate::test_support::Scratch;

/// 一份什么都没设的快照：`MIYU_RESOURCES` 和程序的位置照给的。
fn env(miyu_resources: Option<&Path>, exe: Option<&Path>) -> Env {
    Env {
        platform: Platform::current(),
        miyu_home: None,
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: miyu_resources.map(|path| path.as_os_str().to_os_string()),
        exe: exe.map(Path::to_path_buf),
    }
}

/// 源码树里的资源目录：出厂的那一份。
fn repo() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

#[test]
fn miyu_resources_wins_and_must_be_an_absolute_directory() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("res");
    fs::create_dir_all(&dir).unwrap();
    // 设了的照它，程序旁边有没有都不看。
    let exe = scratch.path().join("bin").join("miyu");
    assert_eq!(
        ResourceRoot::locate(&env(Some(&dir), Some(&exe)))
            .unwrap()
            .path(),
        dir
    );
    let relative = ResourceRoot::locate(&env(Some(Path::new("resources")), None));
    assert_eq!(
        relative,
        Err(ResourceError::Relative(PathBuf::from("resources")))
    );
    let missing = scratch.path().join("nowhere");
    assert_eq!(
        ResourceRoot::locate(&env(Some(&missing), None)),
        Err(ResourceError::Missing(missing))
    );
}

#[test]
fn a_leading_tilde_is_the_home() {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.path().join("res")).unwrap();
    let tilde = |home: Option<PathBuf>| Env {
        home,
        ..env(Some(Path::new("~/res")), None)
    };
    // 开头的 `~` 照家目录接，和 `MIYU_HOME` 一样（施工 4-11）。
    let found = ResourceRoot::locate(&tilde(Some(scratch.path().to_path_buf()))).unwrap();
    assert_eq!(found.path(), scratch.path().join("res"));
    // 找不到家目录的：照原样，当相对的报错。
    assert_eq!(
        ResourceRoot::locate(&tilde(None)),
        Err(ResourceError::Relative(PathBuf::from("~/res")))
    );
}

#[test]
fn next_to_the_program_or_in_share_miyu_one_level_up() {
    // 安装脚本：~/.local/lib/miyu/miyu 旁边的 resources/。
    let scratch = Scratch::new();
    let lib = scratch.path().join("lib").join("miyu");
    fs::create_dir_all(lib.join("resources")).unwrap();
    let found = ResourceRoot::locate(&env(None, Some(&lib.join("miyu")))).unwrap();
    assert_eq!(found.path(), lib.join("resources"));
    // deb、rpm、AUR、Homebrew：<前缀>/bin/miyu 和 <前缀>/share/miyu/。
    let prefix = scratch.path().join("usr");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::create_dir_all(prefix.join("share").join("miyu")).unwrap();
    let found = ResourceRoot::locate(&env(None, Some(&prefix.join("bin").join("miyu")))).unwrap();
    assert_eq!(found.path(), prefix.join("share").join("miyu"));
}

#[test]
fn nowhere_says_where_it_looked() {
    let scratch = Scratch::new();
    let bin = scratch.path().join("target").join("debug");
    fs::create_dir_all(&bin).unwrap();
    let error = ResourceRoot::locate(&env(None, Some(&bin.join("miyu")))).unwrap_err();
    assert_eq!(
        error,
        ResourceError::NotFound(vec![
            bin.join("resources"),
            scratch.path().join("target").join("share").join("miyu"),
        ])
    );
    assert!(error.to_string().contains("MIYU_RESOURCES"), "{error}");
    let unknown = ResourceRoot::locate(&env(None, None)).unwrap_err();
    assert_eq!(unknown, ResourceError::NotFound(Vec::new()));
}

#[test]
fn the_engineer_reads_its_one_sentence_and_the_core_texts() {
    let sources = repo().sources("engineer").unwrap();
    assert_eq!(
        sources.persona.persona,
        "You are a helpful software engineer.\n"
    );
    assert!(sources.core.facts.reply_cut.starts_with("<reply-cut>"));
    assert!(
        sources
            .core
            .checkpoint_open
            .contains("conversation-checkpoint")
    );
}

#[test]
fn a_missing_persona_names_the_file_and_a_bad_id_is_refused() {
    let error = repo().sources("nobody").unwrap_err();
    match &error {
        SourceError::Read { path, .. } => {
            assert!(
                path.ends_with(Path::new("personas/nobody/prompts/persona.md")),
                "{path:?}"
            );
        }
        other => panic!("该是读不了：{other:?}"),
    }
    // 说的是英文，写进运行日志（施工 4-9 再补四中）。
    assert!(error.to_string().starts_with("cannot read "), "{error}");
    for bad in ["../core", "Engineer", "", "a/b", "1st"] {
        let refused = repo().sources(bad);
        assert!(
            matches!(refused, Err(SourceError::Persona(_))),
            "「{bad}」该被拒"
        );
        let said = refused
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(said.starts_with("persona id "), "{said}");
        assert!(said.is_ascii(), "{said}");
    }
}
