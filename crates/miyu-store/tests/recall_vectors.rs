//! 检索库里的向量（施工 R-5 下，`docs/blueprint/recall.md` 第三条第 1、2 款）：照（条目、模型）存；缺的照模型列出来、从哪一条
//! 往后列；找最像的几条，带着条目的字和时刻；条目拿掉、换了字、整个来源拿掉，向量跟着没；放向量时条目已经没了的不放；以前的
//! 库（没有这张表）照样开、加上表。

use std::path::PathBuf;

use miyu_kernel::id::Seq;
use miyu_store::recall::{Edit, RecallIndex};

use crate::support::*;

const MODEL: &str = "local:tiny";

fn place(scratch: &Scratch) -> PathBuf {
    root_in(scratch)
        .index(&admin())
        .join("recall")
        .join("test.db")
}

/// 一个放了三条的库。
fn three(scratch: &Scratch) -> RecallIndex {
    let (index, _) = RecallIndex::open(&place(scratch));
    for (key, text) in [
        ("s1/1", "养了一只猫"),
        ("s1/2", "喝乌龙茶"),
        ("s2/1", "周末爬山"),
    ] {
        index.put(key, text, at(1)).expect("放得进");
    }
    index
}

fn keys(missing: &[(i64, String, String)]) -> Vec<&str> {
    missing.iter().map(|(_, key, _)| key.as_str()).collect()
}

#[test]
fn missing_vectors_are_listed_per_model_in_order() {
    let scratch = Scratch::new("vectors-missing");
    let index = three(&scratch);
    let missing = index.missing(MODEL, 0, 10).expect("读得了");
    assert_eq!(keys(&missing), ["s1/1", "s1/2", "s2/1"]);
    assert_eq!(missing[0].2, "养了一只猫", "带着字");
    index
        .put_vector("s1/2", MODEL, &[0.6, 0.8])
        .expect("放得进");
    assert_eq!(
        keys(&index.missing(MODEL, 0, 10).unwrap()),
        ["s1/1", "s2/1"]
    );
    assert_eq!(
        keys(&index.missing("local:other", 0, 10).unwrap()).len(),
        3,
        "照模型分开"
    );
    let after = missing[0].0;
    assert_eq!(
        keys(&index.missing(MODEL, after, 10).unwrap()),
        ["s2/1"],
        "从哪一条往后列"
    );
    assert_eq!(
        keys(&index.missing(MODEL, 0, 1).unwrap()),
        ["s1/1"],
        "最多几条"
    );
}

#[test]
fn the_nearest_come_first_with_their_text() {
    let scratch = Scratch::new("vectors-nearest");
    let index = three(&scratch);
    index.put_vector("s1/1", MODEL, &[1.0, 0.0]).unwrap();
    index.put_vector("s1/2", MODEL, &[0.6, 0.8]).unwrap();
    index.put_vector("s2/1", MODEL, &[0.0, 1.0]).unwrap();
    index
        .put_vector("s2/1", "local:other", &[1.0, 0.0])
        .unwrap();
    let near = index.nearest(MODEL, &[0.8, 0.6], 2).expect("读得了");
    let found: Vec<(&str, f32)> = near
        .iter()
        .map(|hit| (hit.key.as_str(), hit.similar))
        .collect();
    assert_eq!(found.len(), 2, "最多几条");
    assert_eq!(found[0].0, "s1/2");
    assert!((found[0].1 - 0.96).abs() < 1e-6, "{found:?}");
    assert_eq!(found[1].0, "s1/1");
    assert_eq!(near[0].text, "喝乌龙茶");
    assert_eq!(near[0].at, at(1));
    assert!(
        index
            .nearest("local:none", &[1.0, 0.0], 5)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn vectors_go_with_their_items() {
    let scratch = Scratch::new("vectors-go");
    let index = three(&scratch);
    for key in ["s1/1", "s1/2", "s2/1"] {
        index.put_vector(key, MODEL, &[1.0, 0.0]).unwrap();
    }
    index.remove("s2/1").unwrap();
    index.put("s1/2", "改成喝红茶", at(2)).unwrap();
    assert_eq!(
        keys(&index.missing(MODEL, 0, 10).unwrap()),
        ["s1/2"],
        "换了字的向量作废"
    );
    index.forget("s1").unwrap();
    assert!(
        index.nearest(MODEL, &[1.0, 0.0], 10).unwrap().is_empty(),
        "整个来源拿掉"
    );
    let edits = [Edit::Put {
        key: "s3/1".into(),
        text: "看了樱花".into(),
        at: at(3),
    }];
    index.apply("s3", &edits, Seq::new(5).unwrap()).unwrap();
    index.put_vector("s3/1", MODEL, &[1.0, 0.0]).unwrap();
    index
        .apply(
            "s3",
            &[Edit::Remove { key: "s3/1".into() }],
            Seq::new(6).unwrap(),
        )
        .unwrap();
    assert!(
        index.nearest(MODEL, &[1.0, 0.0], 10).unwrap().is_empty(),
        "一批里拿掉的"
    );
    index.put_vector("gone/1", MODEL, &[1.0, 0.0]).unwrap();
    assert!(
        index.nearest(MODEL, &[1.0, 0.0], 10).unwrap().is_empty(),
        "条目没了的不放"
    );
    // 后来放进同一个键（别的字）：没有留下的旧向量，要补。
    index.put("gone/1", "后来的字", at(4)).unwrap();
    assert_eq!(
        keys(&index.missing(MODEL, 0, 10).unwrap()),
        ["gone/1"],
        "不留没主的向量"
    );
    // 拿掉的条目，再用同一个键放进别的字：旧的向量不能算在它头上。
    index.put("s4/1", "第一回的字", at(5)).unwrap();
    index.put_vector("s4/1", MODEL, &[1.0, 0.0]).unwrap();
    index.remove("s4/1").unwrap();
    index.put("s4/1", "第二回的字", at(6)).unwrap();
    assert!(
        keys(&index.missing(MODEL, 0, 10).unwrap()).contains(&"s4/1"),
        "拿掉时向量一起拿掉"
    );
}

#[test]
fn an_older_index_without_the_table_opens_and_gets_it() {
    let scratch = Scratch::new("vectors-older");
    let path = place(&scratch);
    {
        let (index, _) = RecallIndex::open(&path);
        index.put("s1/1", "养了一只猫", at(1)).unwrap();
    }
    // 以前的版本建的库：同一个版本号，没有向量表。
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("DROP TABLE vectors").unwrap();
    drop(db);
    let (index, opened) = RecallIndex::open(&path);
    assert!(
        matches!(opened, miyu_store::recall::Opened::Kept),
        "不重建：墓碑、照到哪都留着：{opened:?}"
    );
    assert_eq!(keys(&index.missing(MODEL, 0, 10).unwrap()), ["s1/1"]);
    index.put_vector("s1/1", MODEL, &[1.0, 0.0]).unwrap();
    assert_eq!(index.nearest(MODEL, &[1.0, 0.0], 1).unwrap().len(), 1);
}
