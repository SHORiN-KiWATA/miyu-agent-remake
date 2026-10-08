//! 顶替（`chat.md` 第四条「守着它的」）：同一个人在窗口里、正好 `window` 以前、别的人；取最晚的一条；接过去和重判；重判
//! 带上前一条接过的几条、照先后；条件合起来同一种只留一个；主触发保留原来的。

use crate::Standing;

use super::super::test_support::{OTHER, SECOND, SENDER, ago, facts, mills, now, person, seq};
use super::super::{Conditions, Hit, Kind, Route, route};
use super::{Pending, Status, Supersede, supersede};

/// 顶替窗口：旧版的 7 秒（18 第七节）。
const WINDOW: i64 = 7 * SECOND;

/// 这一条的序号，和 [`facts`] 的一样。
const THIS: u64 = 1001;

/// 一笔成立了的条件。
fn hit(kind: Kind, bonus: f64) -> Hit {
    Hit { kind, bonus }
}

/// 几笔条件凑成的集合。
fn set(hits: &[Hit]) -> Conditions {
    Conditions {
        hits: hits.to_vec(),
    }
}

/// 发的人在此刻之前 `ms` 毫秒发的序号是 `msg` 的一条，还没回完，没接过别的。
fn pending(msg: u64, ms: i64, status: Status, hits: &[Hit]) -> Pending {
    Pending {
        msg: seq(msg),
        absorbed: Vec::new(),
        sender: person(SENDER),
        at: ago(ms),
        status,
        conditions: set(hits),
    }
}

/// 这一条（[`facts`]，自己成立了 `hits`）照旧版的窗口看顶替。
fn run(hits: &[Hit], pendings: &[Pending]) -> Supersede {
    supersede(&facts(), &set(hits), pendings, now(), WINDOW)
}

/// 被接过或者被取消的是哪一条的序号；没有顶替是 `None`。
fn target(got: &Supersede) -> Option<u64> {
    match got {
        Supersede::None => None,
        Supersede::Inherit { msg, .. } => Some(msg.get()),
        Supersede::Rejudge { cancel, .. } => Some(cancel.get()),
    }
}

/// 合起来的条件；没有顶替是 `None`。
fn merged(got: &Supersede) -> Option<&Conditions> {
    match got {
        Supersede::None => None,
        Supersede::Inherit { conditions, .. } | Supersede::Rejudge { conditions, .. } => {
            Some(conditions)
        }
    }
}

/// 只有刚说过话的一条前一条。
fn chimed() -> Vec<Hit> {
    vec![hit(Kind::AfterSpeaking, 0.1)]
}

#[test]
fn nothing_pending_nothing_superseded() {
    assert_eq!(run(&chimed(), &[]), Supersede::None);
}

#[test]
fn window_is_open_on_the_far_side() {
    let at = |ago| {
        target(&run(
            &chimed(),
            &[pending(995, ago, Status::Committed, &chimed())],
        ))
    };
    assert_eq!(at(0), Some(995), "此刻的算");
    assert_eq!(at(3 * SECOND), Some(995));
    assert_eq!(at(WINDOW - 1), Some(995));
    assert_eq!(at(WINDOW), None, "正好 window 以前的不算");
    assert_eq!(at(WINDOW + 1), None);
    assert_eq!(at(-1), None, "晚于此刻的不算");
}

#[test]
fn only_the_same_sender() {
    let mut other = pending(999, SECOND, Status::Judging, &chimed());
    other.sender = person(OTHER);
    assert_eq!(
        run(&chimed(), &[other.clone()]),
        Supersede::None,
        "别的人不算"
    );
    // 别的人更近，同一个人的在窗口里：接的是同一个人的那条。
    let mine = pending(995, 5 * SECOND, Status::Committed, &chimed());
    assert_eq!(
        target(&run(&chimed(), &[other.clone(), mine.clone()])),
        Some(995)
    );
    assert_eq!(target(&run(&chimed(), &[mine, other])), Some(995));
}

#[test]
fn takes_the_latest() {
    let early = pending(995, 5 * SECOND, Status::Judging, &chimed());
    let late = pending(998, SECOND, Status::Committed, &chimed());
    assert_eq!(target(&run(&[], &[early.clone(), late.clone()])), Some(998));
    assert_eq!(
        target(&run(&[], &[late, early])),
        Some(998),
        "看时刻，不看交进来的先后"
    );
    // 同一毫秒的取交进来靠后的（和第三条施工时定的第 6 条一样）。
    let first = pending(996, 2 * SECOND, Status::Judging, &chimed());
    let second = pending(997, 2 * SECOND, Status::Judging, &chimed());
    assert_eq!(
        target(&run(&[], &[first.clone(), second.clone()])),
        Some(997)
    );
    assert_eq!(target(&run(&[], &[second, first])), Some(996));
}

#[test]
fn committed_is_inherited() {
    let got = run(
        &chimed(),
        &[pending(995, 3 * SECOND, Status::Committed, &chimed())],
    );
    assert_eq!(
        got,
        Supersede::Inherit {
            msg: seq(995),
            conditions: set(&chimed()),
        }
    );
}

#[test]
fn judging_is_rejudged() {
    let got = run(
        &chimed(),
        &[pending(995, 3 * SECOND, Status::Judging, &chimed())],
    );
    assert_eq!(
        got,
        Supersede::Rejudge {
            cancel: seq(995),
            msgs: vec![seq(995), seq(THIS)],
            conditions: set(&chimed()),
        }
    );
}

#[test]
fn rejudge_carries_what_it_absorbed_in_order() {
    let mut before = pending(995, 3 * SECOND, Status::Judging, &chimed());
    before.absorbed = vec![seq(990), seq(993)];
    let Supersede::Rejudge { cancel, msgs, .. } = run(&[], &[before]) else {
        panic!("该重判");
    };
    assert_eq!(cancel, seq(995), "取消的是判官请求挂着的那一条");
    assert_eq!(msgs, [seq(990), seq(993), seq(995), seq(THIS)]);
}

#[test]
fn merged_keeps_one_of_each_kind() {
    let before = [hit(Kind::Direct, 0.3), hit(Kind::AfterSpeaking, 0.1)];
    let own = [
        hit(Kind::Direct, 0.5),
        hit(Kind::Continuation, 0.1),
        hit(Kind::AfterSpeaking, 0.2),
    ];
    let want = set(&[
        hit(Kind::Direct, 0.3),
        hit(Kind::AfterSpeaking, 0.1),
        hit(Kind::Continuation, 0.1),
    ]);
    for status in [Status::Committed, Status::Judging] {
        let got = run(&own, &[pending(995, 3 * SECOND, status, &before)]);
        let conditions = merged(&got).expect("该顶替");
        assert_eq!(
            conditions, &want,
            "{status:?}：前一条的在前，同一种留前一条的"
        );
        assert_eq!(
            mills(conditions.hits.iter().map(|hit| hit.bonus).sum()),
            500
        );
    }
}

#[test]
fn primary_keeps_the_original() {
    // 前一条是续聊，补的这一条只成立了刚说过话：主触发还是续聊，不变成别的。
    let before = [hit(Kind::Continuation, 0.1)];
    let got = run(
        &chimed(),
        &[pending(995, 3 * SECOND, Status::Committed, &before)],
    );
    assert_eq!(
        merged(&got).and_then(Conditions::primary),
        Some(Kind::Continuation)
    );
    // 前一条是路人在刚说过话时的一句，补的这一条 @ 了她：从合起来的集合里取，是冲她来。
    let direct = [hit(Kind::Direct, 0.3)];
    let got = run(
        &direct,
        &[pending(995, 3 * SECOND, Status::Judging, &chimed())],
    );
    let conditions = merged(&got).expect("该重判");
    assert_eq!(conditions.primary(), Some(Kind::Direct));
    assert_eq!(
        route(conditions, Standing::Owner),
        Route::Commit,
        "合起来的照第三条走路"
    );
}

#[test]
fn own_nothing_still_supersedes() {
    // 补的这一条自己一个条件都没有（发错了马上改，常常没 @ 她）：照样接过去，条件是前一条的。
    let before = [hit(Kind::Direct, 0.3)];
    let got = run(&[], &[pending(995, 3 * SECOND, Status::Committed, &before)]);
    assert_eq!(
        got,
        Supersede::Inherit {
            msg: seq(995),
            conditions: set(&before),
        }
    );
}
