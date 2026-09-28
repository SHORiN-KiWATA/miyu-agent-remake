use std::path::Path;

use super::Home;

fn home(path: &str) -> Home {
    Home::new(Some(Path::new(path)))
}

#[test]
fn the_home_itself_and_paths_under_it_become_tilde() {
    let home = home("/home/ai");
    assert_eq!(home.shorten("/home/ai"), "~");
    assert_eq!(home.shorten("/home/ai/.miyu"), "~/.miyu");
    assert_eq!(
        home.shorten("读不了 /home/ai/.miyu/x：没有权限"),
        "读不了 ~/.miyu/x：没有权限"
    );
    assert_eq!(home.shorten("\"/home/ai/a b\""), "\"~/a b\"");
    // 一段里有几处，都换。
    assert_eq!(home.shorten("/home/ai/a -> /home/ai/b"), "~/a -> ~/b");
}

#[test]
fn other_directories_that_only_look_alike_stay() {
    let home = home("/home/ai");
    // 前缀一样的别的目录。
    assert_eq!(home.shorten("/home/aim/x"), "/home/aim/x");
    assert_eq!(home.shorten("/home/ai.old"), "/home/ai.old");
    assert_eq!(home.shorten("/home/ai_2"), "/home/ai_2");
    assert_eq!(home.shorten("/home/ai-2"), "/home/ai-2");
    // 前面还接着路径：是别处的一截。
    assert_eq!(home.shorten("/srv/home/ai/x"), "/srv/home/ai/x");
    assert_eq!(home.shorten("a/home/ai"), "a/home/ai");
    // 凑巧一样的一截后面跟着别的名字，前面那处照换：找下去不会停。
    assert_eq!(home.shorten("/home/aim /home/ai"), "/home/aim ~");
}

#[test]
fn a_home_written_with_a_trailing_separator_is_the_same_home() {
    assert_eq!(home("/home/ai/").shorten("/home/ai/x"), "~/x");
    assert_eq!(home("/home/ai//").shorten("/home/ai"), "~");
}

#[test]
fn windows_paths_and_the_verbatim_prefix() {
    let home = home(r"C:\Users\ai");
    assert_eq!(home.shorten(r"C:\Users\ai\.miyu"), r"~\.miyu");
    assert_eq!(home.shorten(r"\\?\C:\Users\ai\.miyu"), r"~\.miyu");
    assert_eq!(home.shorten(r"C:\Users\aim"), r"C:\Users\aim");
}

#[test]
fn no_home_an_empty_one_or_the_root_changes_nothing() {
    for none in [
        Home::new(None),
        home(""),
        home("/"),
        home(r"C:\"),
        home("C:"),
    ] {
        assert_eq!(none.shorten("/home/ai/x"), "/home/ai/x");
        assert_eq!(none.shorten(r"C:\x"), r"C:\x");
    }
}
