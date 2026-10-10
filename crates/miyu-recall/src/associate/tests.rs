//! 联想怎么挑（施工 R-8）：门槛表的读法和写错的几种；过门槛的照先后、交过的不要、一轮最多三条、一段的名额用完不带。

use std::collections::BTreeSet;

use super::{SEGMENT, TURN, Thresholds, pick};

#[test]
fn thresholds_are_read_per_model() {
    let read = Thresholds::parse(
        "# 注释\n[thresholds]\n\"local:bge-small-zh-v1.5\" = 0.6\n\"a/emb\" = 0.75\n",
    )
    .expect("合写法");
    assert_eq!(read.of("local:bge-small-zh-v1.5"), Some(0.6));
    assert_eq!(read.of("a/emb"), Some(0.75));
    assert_eq!(read.of("local:tiny"), None, "表里没有的不联想");
    assert_eq!(
        Thresholds::parse("[thresholds]\n")
            .expect("空表合写法")
            .of("x"),
        None
    );
}

#[test]
fn bad_threshold_tables_say_why() {
    for (text, why) in [
        ("[thresholds]\n\"m\" = 1.5\n", "1.5"),
        ("[thresholds]\n\"m\" = -1.5\n", "-1.5"),
        ("[thresholds]\n\"m\" = \"高\"\n", "line 2"),
        ("[other]\n", "other"),
        ("[thresholds\n", "line 1"),
    ] {
        let error = Thresholds::parse(text).expect_err(text);
        assert!(error.contains(why), "{text}：{error}");
    }
}

fn near(pairs: &[(&'static str, f32)]) -> Vec<(&'static str, f32)> {
    pairs.to_vec()
}

#[test]
fn only_what_passes_the_threshold_in_order() {
    let found = near(&[("m3", 0.81), ("m1", 0.66), ("m9", 0.60), ("m2", 0.59)]);
    assert_eq!(
        pick(&found, 0.60, &BTreeSet::new(), 0),
        ["m3", "m1", "m9"],
        "正好在门槛上的算过"
    );
    assert!(pick(&found, 0.9, &BTreeSet::new(), 0).is_empty());
    assert!(pick::<&str>(&[], 0.1, &BTreeSet::new(), 0).is_empty());
}

#[test]
fn already_brought_ones_and_the_caps() {
    let found = near(&[
        ("m1", 0.9),
        ("m2", 0.8),
        ("m3", 0.7),
        ("m4", 0.65),
        ("m5", 0.62),
    ]);
    let seen: BTreeSet<&str> = ["m2"].into_iter().collect();
    assert_eq!(
        pick(&found, 0.6, &seen, 0),
        ["m1", "m3", "m4"],
        "交过的不要，一轮最多 {TURN} 条"
    );
    assert_eq!(
        pick(&found, 0.6, &seen, SEGMENT - 2),
        ["m1", "m3"],
        "这一段只剩两个名额"
    );
    assert!(pick(&found, 0.6, &seen, SEGMENT).is_empty(), "名额用完了");
    assert!(pick(&found, 0.6, &seen, SEGMENT + 3).is_empty());
}

#[test]
fn the_shipped_table_reads() {
    let shipped = Thresholds::parse(include_str!(
        "../../../../resources/core/memory/recall.toml"
    ))
    .expect("出厂的合写法");
    assert_eq!(shipped.of("local:bge-small-zh-v1.5"), Some(0.60));
}
