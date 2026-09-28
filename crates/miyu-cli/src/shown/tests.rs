use std::ffi::OsStr;

use super::colored;

#[test]
fn no_color_counts_only_when_set_and_not_empty() {
    // 终端上：没设、设成空的上色；设了不空的不上色，写什么都一样（no-color.org）。
    assert!(colored(true, None));
    assert!(colored(true, Some(OsStr::new(""))));
    assert!(!colored(true, Some(OsStr::new("1"))));
    assert!(!colored(true, Some(OsStr::new("0"))));
    // 不是终端的从不上色。
    assert!(!colored(false, None));
    assert!(!colored(false, Some(OsStr::new(""))));
}
