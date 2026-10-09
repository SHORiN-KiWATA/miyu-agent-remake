//! 她被禁言到什么时候（施工 O-25 中，`onebot.md` 第一条「群里怎么叫她」第 2 条、「出站队列」第 7 条）：照最后一条
//! `ext.onebot.venues.muted` 的 `until`，到了（不晚于此刻）就不算；之后有 `unmuted` 的不算；再禁一次照新的；读不出 `until` 的
//! 不改。桥重启以后补来的（`upto` 以内）照样收。夹具在 `tests.rs`。

use serde_json::{Value, json};

use super::tests::{at, event, take_all};
use super::{MUTED, Projection, UNMUTED};

/// 第 `seq` 条：桥记的禁言、解禁，`body` 照给的。
fn noted(seq: u64, kind: &str, body: Value) -> miyu_kernel::event::Event {
    event(
        seq,
        0,
        kind,
        None,
        json!({"kind": "module", "id": "onebot"}),
        body,
    )
}

#[test]
fn muted_until_the_time_or_the_lift() {
    for upto in [0, 9] {
        let mut projection = Projection::new(upto);
        assert_eq!(projection.muted(at(0)), None, "没禁过");
        take_all(
            &mut projection,
            vec![noted(1, MUTED, json!({"until": at(100)}))],
        );
        assert_eq!(projection.muted(at(50)), Some(at(100)));
        assert_eq!(projection.muted(at(100)), None, "到了就不算（upto {upto}）");
        take_all(&mut projection, vec![noted(2, UNMUTED, json!({}))]);
        assert_eq!(projection.muted(at(50)), None, "解禁了");
        take_all(
            &mut projection,
            vec![noted(3, MUTED, json!({"until": at(200)}))],
        );
        assert_eq!(projection.muted(at(150)), Some(at(200)), "再禁一次照新的");
        take_all(
            &mut projection,
            vec![noted(4, MUTED, json!({"until": at(120)}))],
        );
        assert_eq!(projection.muted(at(150)), None, "改短了照最后一条");
        take_all(
            &mut projection,
            vec![noted(5, MUTED, json!({"until": "不是时刻"}))],
        );
        assert_eq!(projection.muted(at(110)), Some(at(120)), "读不出的不改");
    }
}
