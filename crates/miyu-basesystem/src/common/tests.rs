//! 共用的几样：当没传的几种写法、Windows 的前缀、相近的名字怎么算；数是 1 的时候编号多接 `/one`
//! （施工 4-5 再补）。

use std::path::Path;

use super::given;
use super::shown::plain;
use super::similar::closeness;
use super::{said, said_n};

#[test]
fn undefined_null_and_empty_are_not_given() {
    for value in ["", "undefined", "null"] {
        assert_eq!(given(Some(value.to_string())), None, "{value:?}");
    }
    assert_eq!(given(None), None);
    assert_eq!(given(Some("src".to_string())), Some("src".to_string()));
    // 只认这三种写法，别的照原样。
    assert_eq!(given(Some("NULL".to_string())), Some("NULL".to_string()));
}

#[test]
fn the_verbatim_prefix_of_windows_is_dropped() {
    assert_eq!(plain(Path::new(r"\\?\C:\work\a.rs")), r"C:\work\a.rs");
    assert_eq!(plain(Path::new(r"\\?\UNC\host\share\a")), r"\\host\share\a");
    assert_eq!(plain(Path::new("/home/u/a.rs")), "/home/u/a.rs");
}

#[test]
fn near_names_are_case_stem_or_a_couple_of_edits() {
    // 只差大小写。
    assert_eq!(closeness("readme.md", "readme.md"), Some(0));
    // 主名一样，扩展名不同；没有扩展名的也算。
    assert_eq!(closeness("main.rs", "main.ts"), Some(1));
    assert_eq!(closeness("readme", "readme.md"), Some(1));
    // `.` 打头的整个是主名：`.gitignore` 和 `.gitattributes` 不算主名一样。
    assert_eq!(closeness(".gitignore", ".gitattributes"), None);
    // 改一两个字。
    assert_eq!(closeness("config.toml", "confg.toml"), Some(2));
    assert_eq!(closeness("config.toml", "cnofig.toml"), Some(3));
    assert_eq!(closeness("config.toml", "other.toml"), None);
    // 太短的不照改几个字算。
    assert_eq!(closeness("a.c", "b.c"), None);
}

#[test]
fn said_n_routes_to_one_only_when_the_number_is_one() {
    assert_eq!(
        said_n("read/lines", "count", 1),
        said("read/lines/one").with("count", "1")
    );
    assert_eq!(
        said_n("read/lines", "count", 0),
        said("read/lines").with("count", "0")
    );
    assert_eq!(
        said_n("read/lines", "count", 2),
        said("read/lines").with("count", "2")
    );
    // 大的数也照旧，不是只挡住 0、2 两个边界。
    assert_eq!(
        said_n("read/lines", "count", 37),
        said("read/lines").with("count", "37")
    );
}
