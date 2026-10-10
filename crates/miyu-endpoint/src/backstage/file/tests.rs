//! 后台页里的相对路径怎么拆（施工 F-6 中）。

use super::segments;

#[test]
fn paths_inside_the_page_are_split() {
    assert_eq!(segments(""), Some(vec!["index.html".to_string()]));
    assert_eq!(segments("app.js"), Some(vec!["app.js".to_string()]));
    assert_eq!(
        segments("assets/图标.svg"),
        Some(vec!["assets".to_string(), "图标.svg".to_string()])
    );
}

#[test]
fn paths_that_could_leave_the_page_are_refused() {
    for path in [
        "/etc/passwd",
        "../x",
        "a/../b",
        "./a",
        "a//b",
        "a/",
        "a\\b",
        "c:x",
        "a\nb",
    ] {
        assert_eq!(segments(path), None, "{path:?}");
    }
}
