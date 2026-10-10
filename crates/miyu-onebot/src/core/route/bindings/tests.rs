//! 问到的「是不是终端管理员」（施工 O-31）：记下的取得到，过了 `binding_seconds` 的要再问；再记一次盖掉原来的；0 是每次都问。

use std::time::{Duration, Instant};

use super::Bindings;

#[test]
fn an_answer_lasts_as_long_as_it_is_kept() {
    let keep = Duration::from_secs(60);
    let start = Instant::now();
    let mut bindings = Bindings::new(keep);
    assert_eq!(bindings.known("qq:10001", start), None, "没问过");
    bindings.note("qq:10001", true, start);
    bindings.note("qq:20002", false, start);
    assert_eq!(bindings.known("qq:10001", start), Some(true));
    assert_eq!(bindings.known("qq:20002", start), Some(false), "不是的也记");
    let late = start + keep - Duration::from_millis(1);
    assert_eq!(bindings.known("qq:10001", late), Some(true));
    assert_eq!(bindings.known("qq:10001", start + keep), None, "过了的再问");
    bindings.note("qq:10001", false, start + keep);
    assert_eq!(
        bindings.known("qq:10001", start + keep),
        Some(false),
        "照新的"
    );
    let mut every = Bindings::new(Duration::ZERO);
    every.note("qq:10001", true, start);
    assert_eq!(every.known("qq:10001", start), None, "0 是每次都问");
}
