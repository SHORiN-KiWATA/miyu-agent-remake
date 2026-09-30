//! 压后重建的清单（施工 6-5）：最多 30 个、多的写还有几个，后面是取回指路。

use super::*;

/// 短的模板，一眼认得出。
fn notes() -> Notes {
    let template = |source: &str| Template::parse(source).unwrap();
    Notes {
        files: template("<files>\n"),
        files_more: template("<more {count}/>\n"),
        retrieve: template("<retrieve {upto}/>\n"),
        too_large: template("<too-large {files}/>\n"),
        uncovered: None,
    }
}

#[test]
fn the_list_stops_at_thirty_and_says_how_many_more() {
    let notes = notes();
    let listed: Vec<String> = (0..32).map(|n| format!("f{n}.rs")).collect();
    let text = files_and_retrieve(&notes, &listed, Seq::new(9).unwrap());
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "<files>");
    assert_eq!(lines[1], "- f0.rs");
    assert_eq!(lines[30], "- f29.rs");
    assert_eq!(lines[31], "<more 2/>");
    assert_eq!(lines[32], "<retrieve 9/>");
    assert_eq!(lines.len(), 33);
    // 没读过文件的只有取回指路。
    assert_eq!(
        files_and_retrieve(&notes, &[], Seq::new(3).unwrap()),
        "<retrieve 3/>\n"
    );
}
