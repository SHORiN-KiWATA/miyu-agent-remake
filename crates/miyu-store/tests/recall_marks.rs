//! 检索库照到的位置怎么挪（施工 R-2 下，`docs/blueprint/recall.md` 的 `apply`）：落后的一批不写、埋了墓碑的会话不再写、
//! 删会话先埋再拿掉。后台补旧会话读日志在前、写在后，这几条挡着它把撤销了的、删掉的又放回去。

use std::path::PathBuf;

use miyu_kernel::id::{Seq, SessionId};
use miyu_store::recall::{Edit, RecallIndex, RecallIndexes, Room};

use crate::support::*;

/// 临时数据根里管理员的一个检索库的位置。
fn place(scratch: &Scratch) -> PathBuf {
    root_in(scratch)
        .index(&admin())
        .join("recall")
        .join("test.db")
}

fn seq(n: u64) -> Seq {
    Seq::new(n).expect("序号从 1 起")
}

/// 照到的位置不往回挪（施工 R-2 下，`recall.md`）：照到的位置已经在这一批以后的，整批不写；一样的照写（同样的事件，改的也
/// 一样）。后台补旧会话读日志在前、写在后，会话自己这时可能已经往前写了：落后的一批不该把撤销了的又放回去。
#[test]
fn a_batch_behind_where_its_source_got_to_is_not_written() {
    let scratch = Scratch::new("recall-behind");
    let (index, _) = RecallIndex::open(&place(&scratch));
    let put = |key: &str, text: &str| Edit::Put {
        key: key.into(),
        text: text.into(),
        at: at(1),
    };
    index
        .apply("s1", &[put("s1/3", "今天看了樱花")], seq(10))
        .unwrap();
    index
        .apply(
            "s1",
            &[
                Edit::Remove { key: "s1/3".into() },
                Edit::Bury { key: "s1/3".into() },
            ],
            seq(12),
        )
        .unwrap();
    index
        .apply("s1", &[put("s1/3", "今天看了樱花")], seq(10))
        .unwrap();
    assert!(
        index.keys().unwrap().is_empty(),
        "落后的一批没把撤销了的放回去"
    );
    assert!(index.is_buried("s1/3").unwrap(), "墓碑还在");
    assert_eq!(
        index.mark("s1").unwrap(),
        Some(seq(12)),
        "照到的位置没往回挪"
    );
    index
        .apply("s1", &[put("s1/5", "明天去爬山")], seq(12))
        .unwrap();
    assert_eq!(index.keys().unwrap(), ["s1/5"], "一样的照写");
    index
        .apply("s2", &[put("s2/1", "别的来源")], seq(3))
        .unwrap();
    assert_eq!(index.keys().unwrap(), ["s1/5", "s2/1"], "各来源各算");
}

/// 整个会话埋了墓碑的（删掉了），它的一批不再写（施工 R-2 下）：补齐旧会话的线程读日志在前、写在后，删会话夹在中间的，
/// 不该把删掉的会话的对话又写回去。
#[test]
fn a_buried_session_gets_nothing_more() {
    let scratch = Scratch::new("recall-buried-source");
    let (index, _) = RecallIndex::open(&place(&scratch));
    index.bury("s1/").unwrap();
    index
        .apply(
            "s1",
            &[Edit::Put {
                key: "s1/3".into(),
                text: "删掉的会话说过的".into(),
                at: at(1),
            }],
            seq(5),
        )
        .unwrap();
    assert!(index.keys().unwrap().is_empty(), "埋了的会话不再写");
    assert_eq!(index.mark("s1").unwrap(), None, "照到的位置也不记");
    index
        .apply(
            "s10",
            &[Edit::Put {
                key: "s10/3".into(),
                text: "别的会话".into(),
                at: at(1),
            }],
            seq(5),
        )
        .unwrap();
    assert_eq!(
        index.keys().unwrap(),
        ["s10/3"],
        "只看自己那一块：s1/ 不碍着 s10"
    );
}

/// 删会话先埋墓碑、再拿掉（施工 R-2 下）：拿掉以后、埋以前写进来的一批也挡得住。
#[test]
fn forgetting_a_session_buries_it_first() {
    let scratch = Scratch::new("recall-forget-order");
    let root = root_in(&scratch);
    let indexes = RecallIndexes::new(&root);
    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").unwrap();
    let room = Room::persona(&admin(), "engineer");
    let (index, _) = indexes.turns(&room);
    let source = session.to_string();
    index
        .apply(
            &source,
            &[Edit::Put {
                key: format!("{source}/3"),
                text: "说过的".into(),
                at: at(1),
            }],
            seq(3),
        )
        .unwrap();
    indexes.forget_session(&admin(), &session).unwrap();
    assert!(index.keys().unwrap().is_empty());
    assert!(index.is_buried(&format!("{source}/")).unwrap());
    index
        .apply(
            &source,
            &[Edit::Put {
                key: format!("{source}/3"),
                text: "说过的".into(),
                at: at(1),
            }],
            seq(3),
        )
        .unwrap();
    assert!(index.keys().unwrap().is_empty(), "删掉以后补进来的一批不写");
}
