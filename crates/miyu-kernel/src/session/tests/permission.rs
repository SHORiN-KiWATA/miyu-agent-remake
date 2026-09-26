//! 切权限级别（`docs/designs/02-内核.md` 第六节「权限级别怎么切」）：空闲时切；切到一样的；不认识的
//! 级别；请求在路上时收紧、放宽；工具在跑时收紧；拦下以后这一步齐了；一步里来回切；回合开头切；
//! 挂接点跑完以后切；收紧成工作区；只读开着时改常用的那一级；环境变了以后切。

use super::executor::*;
use super::*;
use crate::event::{Level, PolicyChanged, ToolStatus};

/// 编号是 `n` 的命令：alice 切权限级别，改哪样写哪样。
pub(super) fn switch(n: u64, level: Option<Level>, read_only: Option<bool>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::SetPermission { level, read_only },
    })
}

/// 编号是 `n` 的命令：alice 开关只读。
pub(super) fn read_only(n: u64, on: bool) -> Input {
    switch(n, None, Some(on))
}

/// 替身的模板写出来的权限那一块。
fn level_fact(level: &str) -> String {
    format!(r#"<p l="{level}"/>"#)
}

/// 一条只换了权限的 `session.policy_changed`：换成的权限。
fn switched_to(event: &Event) -> &Permission {
    match &event.body {
        Body::PolicyChanged(PolicyChanged {
            policy: None,
            permission: Some(permission),
        }) => permission,
        body => panic!("应该是只换了权限的 session.policy_changed：{body:?}"),
    }
}

fn permission(level: Level, read_only: bool) -> Permission {
    Permission { level, read_only }
}

/// 第 `seen` 号请求交给了执行器。
fn asks(actions: &[Action], seen: u64) -> bool {
    calls(actions).first().map(|(seen, _)| *seen) == Some(seq(seen))
}

#[test]
fn switching_while_idle_is_recorded_and_injected_when_the_next_turn_opens() {
    let mut session = asking();
    answer(&mut session, 5, "好");
    session.handle(stored(8));
    let actions = session.handle(read_only(2, true));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[9]));
    assert_eq!(
        switched_to(&events[0]),
        &permission(Level::Workspace, true),
        "两格都写"
    );
    assert_eq!(
        (
            events[0].by.clone(),
            events[0].cause.clone(),
            events[0].turn
        ),
        (alice(), Some(id(2)), None)
    );
    assert_eq!(
        replies(&session.handle(stored(9))),
        [&accepted_reply(2, &[9])]
    );
    // 下一个回合开始时注入只读的那一块；环境没变，那一块不注入。
    let actions = session.handle(send(3, "再看看"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[10, 11, 12]));
    assert_eq!(fact_of(&events[2]).text, level_fact("read_only"));
}

#[test]
fn switching_to_the_same_level_is_accepted_and_records_nothing() {
    let mut session = session();
    assert_eq!(
        session.handle(read_only(1, false)),
        [accepted_reply(1, &[])]
    );
    assert_eq!(
        session.handle(switch(2, Some(Level::Workspace), None)),
        [accepted_reply(2, &[])]
    );
    assert_eq!(
        session.handle(switch(3, None, None)),
        [accepted_reply(3, &[])]
    );
    assert_eq!(
        session.handle(read_only(1, false)),
        [accepted_reply(1, &[])],
        "同一个编号再来，照上一次回应"
    );
    assert_eq!(
        appended(&session.handle(send(4, "hi"))).first(),
        Some(&seq(2))
    );
}

#[test]
fn an_unknown_level_is_rejected() {
    let mut session = session();
    let root = Some(Level::Other("root".to_string()));
    assert_eq!(
        session.handle(switch(1, root, Some(true))),
        [Action::Reply {
            id: id(1),
            outcome: Outcome::Rejected {
                reason: Reason::UnknownLevel
            },
        }]
    );
    assert_eq!(Reason::UnknownLevel.code(), "unknown_level");
    // 只读那一格也没改：开回合时还是工作区。
    let actions = session.handle(send(2, "hi"));
    assert_eq!(appended(&actions), seqs(&[2, 3, 4, 5]));
    assert_eq!(
        fact_of(&appended_events(&actions)[3]).text,
        level_fact("workspace")
    );
}

#[test]
fn tightening_while_asking_denies_the_writes_in_the_reply() {
    let mut session = asking();
    let actions = session.handle(read_only(2, true));
    assert_eq!(appended(&actions), seqs(&[6]));
    assert_eq!(
        appended_events(&actions)[0].turn,
        Some(turn3()),
        "回合进行中切的带上这个回合"
    );
    let actions = call_tools(
        &mut session,
        5,
        &[("read", "{}"), ("write", "{}"), ("shell", "{}")],
    );
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[7, 8, 9]));
    assert_eq!(
        result_of(&events[2]),
        (
            call(7, 2),
            ToolStatus::Denied,
            By::Kernel,
            "read only".to_string()
        )
    );
    assert_eq!(events[2].cause, Some(id(1)));
    // 读的、跑命令的照常派：命令在只读的沙盒里跑。
    assert_eq!(ran(&session.handle(stored(9))), [call(7, 1)]);
    assert_eq!(ran(&session.handle(done(call(7, 1), "a"))), [call(7, 3)]);
}

#[test]
fn tightening_while_tools_run_denies_the_waiting_writes() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", "{}"), ("write", "{}")]);
    assert_eq!(ran(&session.handle(stored(7))), [call(6, 1)]);
    let actions = session.handle(read_only(2, true));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8, 9]));
    assert_eq!(
        result_of(&events[1]),
        (
            call(6, 2),
            ToolStatus::Denied,
            By::Kernel,
            "read only".to_string()
        )
    );
    assert!(ran(&actions).is_empty() && calls(&actions).is_empty());
    // 读的跑完了，这一步齐了：只读的那一块排在结果后面，然后请求。
    let actions = session.handle(done(call(6, 1), "a"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[10, 11]));
    assert_eq!(fact_of(&events[1]).text, level_fact("read_only"));
    assert_eq!(
        (
            events[1].by.clone(),
            events[1].cause.clone(),
            events[1].turn
        ),
        (By::Kernel, Some(id(1)), Some(turn3()))
    );
    assert!(asks(&session.handle(stored(11)), 11));
}

#[test]
fn a_step_left_with_nothing_to_run_asks_again() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("write", "{}")]);
    // 回复还没落盘，调用还没派：拦下了就齐了，当场查事实。
    let actions = session.handle(read_only(2, true));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8, 9, 10]));
    assert_eq!(result_of(&events[1]).1, ToolStatus::Denied);
    assert_eq!(fact_of(&events[2]).text, level_fact("read_only"));
    let actions = session.handle(stored(10));
    assert!(ran(&actions).is_empty(), "拦下的不派");
    assert!(asks(&actions, 10));
}

#[test]
fn loosening_while_asking_takes_effect_at_the_next_request() {
    let mut session = session();
    session.handle(read_only(1, true));
    assert_eq!(
        appended(&session.handle(send(2, "hi"))),
        seqs(&[3, 4, 5, 6])
    );
    session.handle(stored(6));
    session.handle(hooks_done(TurnId::new(seq(4)), Vec::new()));
    assert_eq!(appended(&session.handle(read_only(3, false))), seqs(&[7]));
    // 这次回复里写文件的还是拦下；这一步齐了，换回工作区，注入一块。
    let actions = call_tools(&mut session, 6, &[("write", "{}")]);
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8, 9, 10, 11]));
    assert_eq!(result_of(&events[2]).1, ToolStatus::Denied);
    assert_eq!(fact_of(&events[3]).text, level_fact("workspace"));
    assert!(asks(&session.handle(stored(11)), 11));
    // 之后写文件照常派。
    call_tools(&mut session, 11, &[("write", "{}")]);
    assert_eq!(ran(&session.handle(stored(13))), [call(12, 1)]);
}

#[test]
fn switching_back_and_forth_within_a_step_injects_nothing() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", "{}")]);
    session.handle(stored(7));
    assert_eq!(appended(&session.handle(read_only(2, true))), seqs(&[8]));
    assert_eq!(appended(&session.handle(read_only(3, false))), seqs(&[9]));
    let actions = session.handle(done(call(6, 1), "a"));
    assert_eq!(appended(&actions), seqs(&[10]), "和最近那一块一样，不注入");
    assert!(asks(&session.handle(stored(10)), 10));
}

#[test]
fn a_switch_at_the_start_of_a_turn_is_injected_after_the_hooks() {
    let mut session = session();
    session.handle(send(1, "hi"));
    assert_eq!(appended(&session.handle(read_only(2, true))), seqs(&[6]));
    session.handle(stored(6));
    let actions = session.handle(hooks_done(turn3(), vec![injection("memory", "<memory/>")]));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[7, 8]));
    assert_eq!(fact_of(&events[1]).text, level_fact("read_only"));
    assert_eq!(events[1].by, By::Kernel);
    assert!(asks(&session.handle(stored(8)), 8));
}

#[test]
fn a_switch_after_the_hooks_is_checked_on_the_spot() {
    // 挂接点跑完了、还没落盘时切的：当场查，排在切的后面。
    let mut session = session();
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), vec![injection("memory", "<memory/>")]));
    let actions = session.handle(read_only(2, true));
    assert_eq!(appended(&actions), seqs(&[7, 8]));
    assert_eq!(
        fact_of(&appended_events(&actions)[1]).text,
        level_fact("read_only")
    );
    assert!(asks(&session.handle(stored(8)), 8));
}

#[test]
fn tightening_to_workspace_denies_nothing_and_leaves_running_calls_alone() {
    let mut session = session();
    session.handle(switch(1, Some(Level::Full), None));
    session.handle(send(2, "hi"));
    session.handle(stored(6));
    session.handle(hooks_done(TurnId::new(seq(4)), Vec::new()));
    call_tools(&mut session, 6, &[("read", "{}"), ("write", "{}")]);
    assert_eq!(ran(&session.handle(stored(8))), [call(7, 1)]);
    // 完全放开收紧成工作区：只读才拦写文件的。
    assert_eq!(
        appended(&session.handle(switch(3, Some(Level::Workspace), None))),
        seqs(&[9])
    );
    assert_eq!(ran(&session.handle(done(call(7, 1), "a"))), [call(7, 2)]);
    // 写文件的已经在跑了，开只读也不动它。
    assert_eq!(appended(&session.handle(read_only(4, true))), seqs(&[11]));
    // 这一步齐了：事实写请求那一刻生效的那一级，中间的工作区不写。
    let actions = session.handle(done(call(7, 2), "ok"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[12, 13]));
    assert_eq!(fact_of(&events[1]).text, level_fact("read_only"));
}

#[test]
fn changing_the_usual_level_under_read_only_injects_nothing() {
    let mut session = session();
    session.handle(read_only(1, true));
    let actions = session.handle(switch(2, Some(Level::Full), None));
    assert_eq!(
        switched_to(&appended_events(&actions)[0]),
        &permission(Level::Full, true),
        "只改常用的那一级，只读照旧"
    );
    let actions = session.handle(send(3, "hi"));
    assert_eq!(
        fact_of(&appended_events(&actions)[3]).text,
        level_fact("read_only")
    );
    session.handle(stored(7));
    session.handle(hooks_done(TurnId::new(seq(5)), Vec::new()));
    call_tools(&mut session, 7, &[("read", "{}")]);
    session.handle(stored(9));
    session.handle(switch(4, Some(Level::Workspace), None));
    let actions = session.handle(done(call(8, 1), "a"));
    assert_eq!(appended(&actions), seqs(&[11]), "实际生效的还是只读");
    assert!(asks(&session.handle(stored(11)), 11));
    // 关掉只读，回到常用的那一级：下一个回合开始时注入。
    session.handle(read_only(5, false));
    answer(&mut session, 11, "好");
    session.handle(stored(15));
    let actions = session.handle(send(6, "再来"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[16, 17, 18]));
    assert_eq!(fact_of(&events[2]).text, level_fact("workspace"));
    // 这一轮里写文件照常派。
    session.handle(stored(18));
    session.handle(hooks_done(TurnId::new(seq(17)), Vec::new()));
    call_tools(&mut session, 18, &[("write", "{}")]);
    assert_eq!(ran(&session.handle(stored(20))), [call(19, 1)]);
}

#[test]
fn the_facts_after_a_switch_keep_the_directory_of_the_turn() {
    let mut session = asking();
    session.handle(Input::Environment(environment("~/elsewhere")));
    session.handle(read_only(2, true));
    call_tools(&mut session, 5, &[("read", "{}")]);
    session.handle(stored(8));
    let actions = session.handle(done(call(7, 1), "a"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[9, 10]), "环境那一块不注入");
    assert_eq!(fact_of(&events[1]).kind.as_str(), "permission");
}
