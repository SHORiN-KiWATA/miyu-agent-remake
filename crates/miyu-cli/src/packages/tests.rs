//! 软件包加的子命令（施工 9-2）：什么时候转交、帮助页多的那一节、没装程序时说什么。真的换成程序、退出码照传，在
//! `crates/miyu/tests/packages.rs` 照真二进制走一遍（换成别的程序会把测试进程换掉）。

use super::*;

fn added(name: &str, program: &str) -> Added {
    Added {
        name: name.to_string(),
        program: program.to_string(),
        about: Phrases::from([
            ("en".to_string(), "Open the terminal interface".to_string()),
            ("zh".to_string(), "打开终端界面".to_string()),
        ]),
        manifest: PathBuf::from("/data/home/admin/packages/tui.toml"),
    }
}

fn args(words: &[&str]) -> Vec<OsString> {
    std::iter::once("miyu")
        .chain(words.iter().copied())
        .map(OsString::from)
        .collect()
}

#[test]
fn only_a_package_command_is_forwarded() {
    let list = [added("tui", "miyu-no-such-program-anywhere")];
    let main = Path::new("/nowhere/miyu");
    let mut err = Vec::new();
    for words in [
        &[][..],
        &["ask", "hi"],
        &["-h"],
        &["web"],
        &["help"],
        &["help", "ask"],
        &["nope"],
    ] {
        assert_eq!(
            forward(&args(words), &list, main, Language::English, &mut err),
            None,
            "{words:?}"
        );
    }
    assert!(err.is_empty());
}

#[test]
fn a_missing_program_is_named_with_its_manifest() {
    let list = [added("tui", "miyu-no-such-program-anywhere")];
    let main = Path::new("/nowhere/miyu");
    for (words, language, said) in [
        (
            &["tui", "--x"][..],
            Language::English,
            "miyu-no-such-program-anywhere not found: /data/home/admin/packages/tui.toml says miyu tui runs it",
        ),
        (
            &["help", "tui"][..],
            Language::Chinese,
            "没找到 miyu-no-such-program-anywhere：清单 /data/home/admin/packages/tui.toml 说 miyu tui 由它跑",
        ),
    ] {
        let mut err = Vec::new();
        assert_eq!(
            forward(&args(words), &list, main, language, &mut err),
            Some(1)
        );
        let text = String::from_utf8(err).unwrap();
        assert!(text.starts_with(said), "{text}");
    }
}

#[test]
fn the_help_page_gets_a_section_after_the_commands() {
    let list = [added("tui", "miyu-tui"), added("onebot", "miyu-onebot")];
    let section = help_section(Language::Chinese, &list);
    assert_eq!(
        section,
        "软件包加的命令：\n  tui                   打开终端界面\n  onebot                打开终端界面\n"
    );
    assert_eq!(
        help_section(Language::English, &list[..1]),
        "Commands from packages:\n  tui                   Open the terminal interface\n"
    );
    assert_eq!(help_section(Language::English, &[]), "");
    let page = "用法：miyu <命令>\n\n命令：\n  ask   说\n\nask 的选项：\n  -c\n";
    assert_eq!(with_section(page, ""), page);
    let with = with_section(page, &section);
    assert!(
        with.contains("  ask   说\n\n软件包加的命令：\n  tui"),
        "接在「命令」那一节后面：{with}"
    );
    assert!(with.contains("打开终端界面\n\nask 的选项："), "{with}");
}
