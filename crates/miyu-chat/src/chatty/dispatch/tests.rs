//! 顶替与分派（`chat.md` 第四条「守着它的」）：同一个人在窗口里、正好 `window` 以前、别的人；取最晚的一条；接过去和
//! 重判；重判带上前一条接过的几条、照先后；条件合起来同一种只留一个；主触发保留原来的。分派的五种；主线闲着时不看
//! 支线；几条支线取第一条；`parallel` 是 0 不分叉；支线满了排队。

use crate::Standing;

use super::super::test_support::{OTHER, SECOND, SENDER, facts, mills, now};
use super::super::{Conditions, Hit, Kind, Route, route};
use super::{Dispatch, Line, Lines, Pending, Status, Supersede, dispatch, supersede};

/// 顶替窗口：旧版的 7 秒（18 第七节）。
const WINDOW: i64 = 7 * SECOND;

/// 第三个人。
const THIRD: &str = "qq:10004";

/// 这一条的消息编号，和 [`facts`] 的一样。
const THIS: &str = "1001";

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

/// 发的人在此刻之前 `ago` 毫秒发的一条，还没回完，没接过别的。
fn pending(msg: &str, ago: i64, status: Status, hits: &[Hit]) -> Pending {
    Pending {
        msg: msg.to_string(),
        absorbed: Vec::new(),
        sender: SENDER.to_string(),
        at: now().now - ago,
        status,
        hits: set(hits),
    }
}

/// 这一条（[`facts`]，自己成立了 `hits`）照旧版的窗口看顶替。
fn run(hits: &[Hit], pendings: &[Pending]) -> Supersede {
    supersede(&facts(), &set(hits), pendings, now(), WINDOW)
}

/// 被接过或者被取消的是哪一条；没有顶替是 `None`。
fn target(got: &Supersede) -> Option<&str> {
    match got {
        Supersede::None => None,
        Supersede::Inherit { msg, .. } => Some(msg),
        Supersede::Rejudge { cancel, .. } => Some(cancel),
    }
}

/// 合起来的条件；没有顶替是 `None`。
fn merged(got: &Supersede) -> Option<&Conditions> {
    match got {
        Supersede::None => None,
        Supersede::Inherit { hits, .. } | Supersede::Rejudge { hits, .. } => Some(hits),
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
            &[pending("995", ago, Status::Committed, &chimed())],
        ))
        .map(str::to_string)
    };
    assert_eq!(at(0).as_deref(), Some("995"), "此刻的算");
    assert_eq!(at(3 * SECOND).as_deref(), Some("995"));
    assert_eq!(at(WINDOW - 1).as_deref(), Some("995"));
    assert_eq!(at(WINDOW), None, "正好 window 以前的不算");
    assert_eq!(at(WINDOW + 1), None);
    assert_eq!(at(-1), None, "晚于此刻的不算");
}

#[test]
fn only_the_same_sender() {
    let mut other = pending("999", SECOND, Status::Judging, &chimed());
    other.sender = OTHER.to_string();
    assert_eq!(
        run(&chimed(), &[other.clone()]),
        Supersede::None,
        "别的人不算"
    );
    // 别的人更近，同一个人的在窗口里：接的是同一个人的那条。
    let mine = pending("995", 5 * SECOND, Status::Committed, &chimed());
    assert_eq!(
        target(&run(&chimed(), &[other.clone(), mine.clone()])),
        Some("995")
    );
    assert_eq!(target(&run(&chimed(), &[mine, other])), Some("995"));
}

#[test]
fn takes_the_latest() {
    let early = pending("995", 5 * SECOND, Status::Judging, &chimed());
    let late = pending("998", SECOND, Status::Committed, &chimed());
    assert_eq!(
        target(&run(&[], &[early.clone(), late.clone()])),
        Some("998")
    );
    assert_eq!(
        target(&run(&[], &[late, early])),
        Some("998"),
        "看时刻，不看交进来的先后"
    );
    // 同一毫秒的取交进来靠后的（和第三条施工时定的第 6 条一样）。
    let first = pending("996", 2 * SECOND, Status::Judging, &chimed());
    let second = pending("997", 2 * SECOND, Status::Judging, &chimed());
    assert_eq!(
        target(&run(&[], &[first.clone(), second.clone()])),
        Some("997")
    );
    assert_eq!(target(&run(&[], &[second, first])), Some("996"));
}

#[test]
fn committed_is_inherited() {
    let got = run(
        &chimed(),
        &[pending("995", 3 * SECOND, Status::Committed, &chimed())],
    );
    assert_eq!(
        got,
        Supersede::Inherit {
            msg: "995".to_string(),
            hits: set(&chimed()),
        }
    );
}

#[test]
fn judging_is_rejudged() {
    let got = run(
        &chimed(),
        &[pending("995", 3 * SECOND, Status::Judging, &chimed())],
    );
    assert_eq!(
        got,
        Supersede::Rejudge {
            cancel: "995".to_string(),
            msgs: vec!["995".to_string(), THIS.to_string()],
            hits: set(&chimed()),
        }
    );
}

#[test]
fn rejudge_carries_what_it_absorbed_in_order() {
    let mut before = pending("995", 3 * SECOND, Status::Judging, &chimed());
    before.absorbed = vec!["990".to_string(), "993".to_string()];
    let Supersede::Rejudge { cancel, msgs, .. } = run(&[], &[before]) else {
        panic!("该重判");
    };
    assert_eq!(cancel, "995", "取消的是判官请求挂着的那一条");
    assert_eq!(msgs, ["990", "993", "995", THIS]);
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
        let got = run(&own, &[pending("995", 3 * SECOND, status, &before)]);
        let hits = merged(&got).expect("该顶替");
        assert_eq!(hits, &want, "{status:?}：前一条的在前，同一种留前一条的");
        assert_eq!(mills(hits.hits.iter().map(|hit| hit.bonus).sum()), 500);
    }
}

#[test]
fn primary_keeps_the_original() {
    // 前一条是续聊，补的这一条只成立了刚说过话：主触发还是续聊，不变成别的。
    let before = [hit(Kind::Continuation, 0.1)];
    let got = run(
        &chimed(),
        &[pending("995", 3 * SECOND, Status::Committed, &before)],
    );
    assert_eq!(
        merged(&got).and_then(Conditions::primary),
        Some(Kind::Continuation)
    );
    // 前一条是路人在刚说过话时的一句，补的这一条 @ 了她：从合起来的集合里取，是冲她来。
    let direct = [hit(Kind::Direct, 0.3)];
    let got = run(
        &direct,
        &[pending("995", 3 * SECOND, Status::Judging, &chimed())],
    );
    let hits = merged(&got).expect("该重判");
    assert_eq!(hits.primary(), Some(Kind::Direct));
    assert_eq!(
        route(hits, Standing::Owner),
        Route::Commit,
        "合起来的照第三条走路"
    );
}

#[test]
fn own_nothing_still_supersedes() {
    // 补的这一条自己一个条件都没有（发错了马上改，常常没 @ 她）：照样接过去，条件是前一条的。
    let before = [hit(Kind::Direct, 0.3)];
    let got = run(
        &[],
        &[pending("995", 3 * SECOND, Status::Committed, &before)],
    );
    assert_eq!(
        got,
        Supersede::Inherit {
            msg: "995".to_string(),
            hits: set(&before),
        }
    );
}

/// 正在回这几个人的一条线。
fn busy(targets: &[&str]) -> Line {
    Line::Busy {
        targets: targets.iter().map(|target| target.to_string()).collect(),
    }
}

/// 一个场所的几条线。
fn lines(main: Line, lanes: Vec<Line>, parallel: u8) -> Lines {
    Lines {
        main,
        lanes,
        parallel,
    }
}

#[test]
fn idle_main_starts_a_round() {
    assert_eq!(
        dispatch(SENDER, &lines(Line::Idle, vec![], 1)),
        Dispatch::StartMain
    );
    // 主线闲着就开主线，就算有支线正在回他（施工时定的第 3 条）。
    let got = dispatch(SENDER, &lines(Line::Idle, vec![busy(&[SENDER])], 1));
    assert_eq!(got, Dispatch::StartMain);
}

#[test]
fn main_round_for_him_is_joined() {
    let got = dispatch(SENDER, &lines(busy(&[OTHER, SENDER]), vec![], 0));
    assert_eq!(got, Dispatch::JoinMain);
    // 主线在回他，支线也在回他：并进主线。
    let got = dispatch(SENDER, &lines(busy(&[SENDER]), vec![busy(&[SENDER])], 1));
    assert_eq!(got, Dispatch::JoinMain);
}

#[test]
fn lane_for_him_is_joined() {
    let got = dispatch(
        SENDER,
        &lines(busy(&[OTHER]), vec![busy(&[THIRD]), busy(&[SENDER])], 2),
    );
    assert_eq!(got, Dispatch::JoinLane(1));
    // 几条都在回他：取第一条。支线已经满了也照样并进去。
    let lanes = vec![busy(&[THIRD]), busy(&[OTHER, SENDER]), busy(&[SENDER])];
    let got = dispatch(SENDER, &lines(busy(&[OTHER]), lanes, 3));
    assert_eq!(got, Dispatch::JoinLane(1));
}

#[test]
fn forks_while_there_is_room() {
    assert_eq!(
        dispatch(SENDER, &lines(busy(&[OTHER]), vec![], 1)),
        Dispatch::Fork
    );
    let got = dispatch(SENDER, &lines(busy(&[OTHER]), vec![busy(&[THIRD])], 2));
    assert_eq!(got, Dispatch::Fork);
}

#[test]
fn queues_when_no_room() {
    // 私聊、串行的群：`parallel` 是 0，不分叉。
    assert_eq!(
        dispatch(SENDER, &lines(busy(&[OTHER]), vec![], 0)),
        Dispatch::Queue
    );
    // 支线满了。
    let got = dispatch(SENDER, &lines(busy(&[OTHER]), vec![busy(&[THIRD])], 1));
    assert_eq!(got, Dispatch::Queue);
    let lanes = vec![busy(&[THIRD]), busy(&[THIRD]), busy(&[THIRD])];
    assert_eq!(
        dispatch(SENDER, &lines(busy(&[OTHER]), lanes, 3)),
        Dispatch::Queue
    );
}
