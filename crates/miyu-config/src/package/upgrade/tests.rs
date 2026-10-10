use super::*;

#[test]
fn the_kind_line_goes_and_its_table_comes() {
    let old = "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = { en = \"P\" }\n\n[command]\nname = \"p\"\nprogram = \"miyu-p\"\nabout = { en = \"P\" }\n";
    assert_eq!(
        without_kind(old).as_deref(),
        Some(
            "[package]\nprotocol = [1, 1]\nname = { en = \"P\" }\n\n[command]\nname = \"p\"\nprogram = \"miyu-p\"\nabout = { en = \"P\" }\n\n[ui]\n"
        )
    );
}

#[test]
fn a_table_already_there_is_not_added_twice() {
    let old = "[package]\nkind = \"process\" # 扩展\nname = { en = \"X\" }\n\n[process]\nargs = [\"serve\"]\n";
    assert_eq!(
        without_kind(old).as_deref(),
        Some("[package]\nname = { en = \"X\" }\n\n[process]\nargs = [\"serve\"]\n")
    );
    let mascot =
        "[package]\nkind = \"mascot\"\nname = { en = \"M\" }\n\n[mascot]\nmodel = \"m.json\"";
    assert_eq!(
        without_kind(mascot).as_deref(),
        Some("[package]\nname = { en = \"M\" }\n\n[mascot]\nmodel = \"m.json\"")
    );
}

#[test]
fn nothing_to_do_or_unknown_is_left_alone() {
    for text in [
        "[package]\nname = { en = \"X\" }\n\n[ui]\n",
        "[package]\nkind = \"daemon\"\n",
        "[package]\nkind = 3\n",
        "package = { kind = \"ui\" }\n",
        "not toml [",
    ] {
        assert_eq!(without_kind(text), None, "{text}");
    }
}
