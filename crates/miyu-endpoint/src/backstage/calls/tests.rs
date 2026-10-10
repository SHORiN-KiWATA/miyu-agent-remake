//! 后台页的方法名（施工 F-6 中）。

use super::valid;

#[test]
fn method_names_are_short_names() {
    for name in ["status", "connection.token", "people.add-one", "a1_b"] {
        assert!(valid(name), "{name}");
    }
    let longest = "a".repeat(64);
    assert!(valid(&longest));
    for name in ["", "Status", "1a", ".a", "a b", "a/b", &"a".repeat(65)] {
        assert!(!valid(name), "{name}");
    }
}
