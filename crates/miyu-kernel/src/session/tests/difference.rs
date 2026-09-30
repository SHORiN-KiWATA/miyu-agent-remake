//! 第一处不同（`docs/designs/08-上下文投影.md` 第七节）：这一次请求和上一次比，前缀从哪里断开。记进
//! `model.called`，也交给执行器，运行日志照它写（施工 3-9 下：项目主人问起「怎么从日志查缓存为什么没命中」）。

use super::executor::*;
use super::*;
use crate::event::Part;
use crate::request::Difference;

#[test]
fn a_second_request_that_only_extends_the_first_has_no_first_difference() {
    let mut session = asking();
    answer(&mut session, 5, "你好");
    session.handle(stored(8));
    session.handle(send(2, "再来"));
    session.handle(stored(10));
    let actions = session.handle(hooks_done(TurnId::new(seq(10)), Vec::new()));
    assert_eq!(
        calls(&actions).first().map(|(seen, _)| *seen),
        Some(seq(10))
    );
    assert!(
        matches!(
            actions.as_slice(),
            [Action::CallModel { changed: None, .. }]
        ),
        "交给执行器的请求动作也说：只是接着加的：{actions:?}"
    );
    let events = appended_events(&answer(&mut session, 10, "好的"));
    let called = called_of(&events[1]);
    assert_eq!(called.messages, 10);
    assert_eq!(called.first_difference, None);
}

/// 替身的组装，system 每次都不一样：前缀从 system 那里断开。
struct Drifting;

impl Assembler for Drifting {
    fn assemble(&self, history: &History) -> Request {
        let mut request = Listing.assemble(history);
        request.system = format!("{} events", history.events().len());
        request
    }

    fn summarize(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request {
        Listing.summarize(history, upto, cut, instructions)
    }

    fn summarize_isolated(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request {
        Listing.summarize_isolated(history, upto, cut, instructions)
    }

    fn summary(&self, reply: &[Block]) -> Option<String> {
        Listing.summary(reply)
    }
}

#[test]
fn a_rewritten_system_is_the_first_difference() {
    let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    let mut policy = policy();
    policy.assembler = Box::new(Drifting);
    let (mut session, _) = Session::create(
        session_id(),
        id(0),
        alice(),
        at(0),
        created,
        policy,
        environment("~/src/miyu"),
    );
    session.handle(stored(1));
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    answer(&mut session, 5, "你好");
    session.handle(stored(8));
    session.handle(send(2, "再来"));
    session.handle(stored(10));
    let actions = session.handle(hooks_done(TurnId::new(seq(10)), Vec::new()));
    assert!(
        matches!(
            actions.as_slice(),
            [Action::CallModel {
                changed: Some(Difference::System),
                ..
            }]
        ),
        "交给执行器的请求动作也带着第一处不同：运行日志照它写：{actions:?}"
    );
    let events = appended_events(&answer(&mut session, 10, "好的"));
    let difference = called_of(&events[1]).first_difference.clone().unwrap();
    assert_eq!(difference.part, Part::System);
}
