//! 清单的图标、后台页（施工 F-6 上，`package-pages.md`「清单多的几格」）。

use crate::package::{Code, PackageKind, read};

/// 一份扩展的清单，`extra` 接在 `[package]` 后面（`[package]` 里的几格）、`tables` 接在最后（别的表）。
fn process(extra: &str, tables: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bridge\" }}\n{extra}\n[command]\nname = \"bridge\"\nprogram = \"miyu-bridge\"\nabout = {{ en = \"Bridge\" }}\n\n[process]\n{tables}"
    )
}

#[test]
fn icon_and_page_are_read() {
    let manifest = read(&process(
        "icon = \"message-circle\"",
        "\n[page]\ndir = \"page\"\n",
    ))
    .unwrap();
    assert_eq!(manifest.icon.as_deref(), Some("message-circle"));
    assert_eq!(manifest.page.as_deref(), Some("page"));
    let manifest = read(&process("", "")).unwrap();
    assert_eq!((manifest.icon, manifest.page), (None, None));
    let builtin = "[package]\nprotocol = [1, 1]\nname = { en = \"Memory\" }\nicon = \"brain\"\n\n[page]\ndir = \"web/page\"\n\n[builtin]\n";
    let manifest = read(builtin).unwrap();
    assert_eq!(manifest.kind, PackageKind::Builtin);
    assert_eq!(manifest.page.as_deref(), Some("web/page"));
}

#[test]
fn a_bad_icon_is_refused_on_its_line() {
    for icon in ["\"\"", "\"Message\"", "\"1a\"", "\"a_b\"", "\"a b\"", "1"] {
        let problem = read(&process(&format!("icon = {icon}"), "")).unwrap_err();
        assert_eq!(
            (problem.code, problem.line),
            (Code::BadIcon, Some(4)),
            "{icon}"
        );
    }
    let long = format!("icon = \"{}\"", "a".repeat(65));
    assert_eq!(read(&process(&long, "")).unwrap_err().code, Code::BadIcon);
    let longest = format!("icon = \"{}\"", "a".repeat(64));
    assert!(read(&process(&longest, "")).is_ok());
}

#[test]
fn a_bad_page_dir_is_refused() {
    for dir in [
        "\"\"",
        "\"/page\"",
        "\"../page\"",
        "\"a/../b\"",
        "\"a\\\\b\"",
        "\"c:page\"",
        "1",
    ] {
        let problem = read(&process("", &format!("\n[page]\ndir = {dir}\n"))).unwrap_err();
        assert_eq!(problem.code, Code::BadPageDir, "{dir}");
    }
    let missing = read(&process("", "\n[page]\n")).unwrap_err();
    assert_eq!(missing.code, Code::MissingKey);
    let unknown = read(&process(
        "",
        "\n[page]\ndir = \"page\"\nentry = \"x.html\"\n",
    ))
    .unwrap_err();
    assert_eq!(unknown.code, Code::UnknownKey);
}

#[test]
fn only_extensions_and_built_in_packages_have_a_page() {
    let ui =
        "[package]\nprotocol = [1, 1]\nname = { en = \"UI\" }\n\n[page]\ndir = \"page\"\n\n[ui]\n";
    assert_eq!(read(ui).unwrap_err().code, Code::WrongKind);
    let worker = "[package]\nprotocol = [1, 1]\nname = { en = \"W\" }\n\n[worker]\nprogram = \"w\"\n\n[page]\ndir = \"page\"\n";
    assert_eq!(read(worker).unwrap_err().code, Code::WrongKind);
}
