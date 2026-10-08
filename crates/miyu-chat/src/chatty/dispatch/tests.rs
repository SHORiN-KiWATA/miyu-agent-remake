//! 分派（`chat.md` 第四条「守着它的」）：五种；主线闲着时不看支线；几条支线取第一条；`parallel` 是 0 不分叉；支线满了
//! 排队。

use miyu_kernel::id::ExternalId;

use super::super::test_support::{OTHER, SENDER, person};
use super::{Dispatch, Line, Lines, dispatch};

/// 第三个人。
const THIRD: &str = "qq:10004";

/// 发这一条的人。
fn sender() -> ExternalId {
    person(SENDER)
}

/// 正在回这几个人的一条线。
fn busy(targets: &[&str]) -> Line {
    Line::Busy {
        targets: targets.iter().map(|target| person(target)).collect(),
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
        dispatch(&sender(), &lines(Line::Idle, vec![], 1)),
        Dispatch::StartMain
    );
    // 主线闲着就开主线，就算有支线正在回他（施工时定的第 3 条）。
    let got = dispatch(&sender(), &lines(Line::Idle, vec![busy(&[SENDER])], 1));
    assert_eq!(got, Dispatch::StartMain);
}

#[test]
fn main_round_for_him_is_joined() {
    let got = dispatch(&sender(), &lines(busy(&[OTHER, SENDER]), vec![], 0));
    assert_eq!(got, Dispatch::JoinMain);
    // 主线在回他，支线也在回他：并进主线。
    let got = dispatch(&sender(), &lines(busy(&[SENDER]), vec![busy(&[SENDER])], 1));
    assert_eq!(got, Dispatch::JoinMain);
}

#[test]
fn lane_for_him_is_joined() {
    let got = dispatch(
        &sender(),
        &lines(busy(&[OTHER]), vec![busy(&[THIRD]), busy(&[SENDER])], 2),
    );
    assert_eq!(got, Dispatch::JoinLane(1));
    // 几条都在回他：取第一条。支线已经满了也照样并进去。
    let lanes = vec![busy(&[THIRD]), busy(&[OTHER, SENDER]), busy(&[SENDER])];
    let got = dispatch(&sender(), &lines(busy(&[OTHER]), lanes, 3));
    assert_eq!(got, Dispatch::JoinLane(1));
}

#[test]
fn forks_while_there_is_room() {
    assert_eq!(
        dispatch(&sender(), &lines(busy(&[OTHER]), vec![], 1)),
        Dispatch::Fork
    );
    let got = dispatch(&sender(), &lines(busy(&[OTHER]), vec![busy(&[THIRD])], 2));
    assert_eq!(got, Dispatch::Fork);
}

#[test]
fn queues_when_no_room() {
    // 私聊、串行的群：`parallel` 是 0，不分叉。
    assert_eq!(
        dispatch(&sender(), &lines(busy(&[OTHER]), vec![], 0)),
        Dispatch::Queue
    );
    // 支线满了。
    let got = dispatch(&sender(), &lines(busy(&[OTHER]), vec![busy(&[THIRD])], 1));
    assert_eq!(got, Dispatch::Queue);
    let lanes = vec![busy(&[THIRD]), busy(&[THIRD]), busy(&[THIRD])];
    assert_eq!(
        dispatch(&sender(), &lines(busy(&[OTHER]), lanes, 3)),
        Dispatch::Queue
    );
}
