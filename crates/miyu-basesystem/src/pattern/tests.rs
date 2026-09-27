//! 模式照 ripgrep 的 `--glob`：不带 `/` 的比文件名，带 `/` 的比路径；绝对路径的模式拆出目录。

use std::path::PathBuf;

use super::{Pattern, split_absolute};

fn matches(pattern: &str, relative: &str) -> bool {
    Pattern::new(pattern).expect("写法对").matches(relative)
}

#[test]
fn without_a_slash_it_is_a_file_name_at_any_depth() {
    assert!(matches("*.rs", "a.rs"));
    assert!(matches("*.rs", "src/deep/b.rs"));
    assert!(!matches("*.rs", "src/b.rs.bak"));
    assert!(matches("Cargo.toml", "crates/x/Cargo.toml"));
}

#[test]
fn with_a_slash_it_is_the_path_from_the_search_dir() {
    assert!(matches("src/*.rs", "src/a.rs"));
    // `*` 不跨目录，`**` 跨。
    assert!(!matches("src/*.rs", "src/deep/a.rs"));
    assert!(matches("src/**/*.rs", "src/deep/a.rs"));
    assert!(matches("src/**/*.rs", "src/a.rs"));
    assert!(!matches("src/*.rs", "other/src/a.rs"));
    // `**/` 开头的在任何一层。
    assert!(matches("**/*.ts", "a.ts"));
    assert!(matches("**/*.ts", "web/app/a.ts"));
    // 开头的 `/` 从搜的目录算起：只认最上面那一层的。
    assert!(matches("/Cargo.toml", "Cargo.toml"));
    assert!(!matches("/Cargo.toml", "crates/x/Cargo.toml"));
}

#[test]
fn braces_and_classes() {
    assert!(matches("*.{ts,tsx}", "src/a.tsx"));
    assert!(matches("*.{ts,tsx}", "a.ts"));
    assert!(!matches("*.{ts,tsx}", "a.js"));
    assert!(matches("[ab].txt", "b.txt"));
    assert!(!matches("[ab].txt", "c.txt"));
}

#[test]
fn a_broken_pattern_says_what_is_wrong() {
    let Err(error) = Pattern::new("[ab") else {
        panic!("[ 没配上 ]");
    };
    assert!(error.contains("unclosed"), "{error}");
}

#[test]
fn an_absolute_pattern_splits_off_its_directory() {
    let root = if cfg!(windows) { r"C:\proj" } else { "/proj" };
    let sep = std::path::MAIN_SEPARATOR;
    assert_eq!(
        split_absolute(&format!("{root}{sep}src{sep}**{sep}*.rs")),
        Some((
            PathBuf::from(format!("{root}{sep}src")),
            "/**/*.rs".to_string()
        ))
    );
    // 一个通配都没有的：拆出上一级目录，只认那一层的这个文件。
    assert_eq!(
        split_absolute(&format!("{root}{sep}Cargo.toml")),
        Some((PathBuf::from(root), "/Cargo.toml".to_string()))
    );
    assert_eq!(
        split_absolute("~/notes/*.md"),
        Some((PathBuf::from("~/notes"), "/*.md".to_string()))
    );
    // Windows 换真实位置得来的路径带 `\\?\` 前缀：前缀里的 `?` 不是通配。
    if cfg!(windows) {
        assert_eq!(
            split_absolute(r"\\?\C:\proj\src\*.rs"),
            Some((PathBuf::from(r"\\?\C:\proj\src"), "/*.rs".to_string()))
        );
    }
    assert_eq!(split_absolute("src/*.rs"), None);
    assert_eq!(split_absolute("*.rs"), None);
}
