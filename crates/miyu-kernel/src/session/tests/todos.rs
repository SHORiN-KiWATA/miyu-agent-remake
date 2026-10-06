//! 待办（施工 D-3，`docs/blueprint/kernel/session.md`「待办」）：换了一份、落了盘才推 `todos.changed`，和上次一样的不推；
//! 撤销写它的那一轮退回更早的一份，恢复回来；载入以后照日志算回来，不重推。

use super::executor::*;
use super::load::{Logged, load};
use super::revert::{revert, unrevert};
use super::*;
use crate::event::{Effect, Todo, TodoStatus, TodoWritten, Transient, TransientBody};
use crate::id::CallId;

fn list(items: &[(&str, TodoStatus)]) -> Vec<Todo> {
    items
        .iter()
        .map(|(content, status)| Todo {
            content: content.to_string(),
            status: status.clone(),
        })
        .collect()
}

/// 调用 `call_id` 跑完了，换上 `todos`。
fn wrote(call_id: CallId, todos: Vec<Todo>) -> Input {
    Input::ToolDone {
        at: at(50),
        call_id,
        error: false,
        blocks: vec![Block::Text(Text {
            text: "updated".to_string(),
        })],
        duration_ms: Some(1),
        human: None,
        effects: vec![Effect::TodoWritten(TodoWritten { todos })],
        stopped: false,
    }
}

/// 推给头的 `todos.changed`，照先后。
fn changes(actions: &[Action]) -> Vec<Vec<Todo>> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::PushTransient(Transient {
                body: TransientBody::TodosChanged(changed),
                ..
            }) => Some(changed.todos.clone()),
            _ => None,
        })
        .collect()
}

/// 请求 `seen` 调一次 `todowrite`，换上 `todos`，都落了盘：交回这一路推的 `todos.changed` 和下一次请求的 `seen`。
fn write(logged: &mut Logged, seen: u64, todos: Vec<Todo>) -> (Vec<Vec<Todo>>, u64) {
    // 测试的策略里工具面只有常用的几件：工具名不要紧，内核只看效果。
    let actions = call_tools(&mut logged.session, seen, &[("write", "{}")]);
    logged.log.extend(appended_events(&actions));
    let actions = logged.allowing(stored(logged.last()));
    let call_id = ran(&actions)[0];
    let mut pushed = changes(&logged.handle(wrote(call_id, todos)));
    let actions = logged.handle(stored(logged.last()));
    pushed.extend(changes(&actions));
    (pushed, calls(&actions).remove(0).0.get())
}

/// 最近一条 `turn.started` 的序号：这一轮的编号。
fn last_turn(logged: &Logged) -> u64 {
    logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| event.seq.get())
        .expect("开过回合")
}

#[test]
fn a_new_list_is_pushed_once_it_is_stored_and_the_same_one_is_not() {
    let a = list(&[
        ("读代码", TodoStatus::Completed),
        ("写测试", TodoStatus::InProgress),
    ]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let (pushed, seen) = write(&mut logged, seen, a.clone());
    assert_eq!(pushed, std::slice::from_ref(&a), "落了盘才推，推一次");
    assert_eq!(logged.session.todos(), a);
    let (pushed, seen) = write(&mut logged, seen, a.clone());
    assert!(pushed.is_empty(), "和上次一样的不推");
    let (pushed, _) = write(&mut logged, seen, Vec::new());
    assert_eq!(pushed, [Vec::<Todo>::new()], "清空了也推");
    assert!(logged.session.todos().is_empty());
}

#[test]
fn undoing_goes_back_to_the_earlier_list_and_redoing_restores_it() {
    let a = list(&[("一", TodoStatus::Pending)]);
    let b = list(&[("一", TodoStatus::Completed), ("二", TodoStatus::Pending)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "第一轮");
    let (_, seen) = write(&mut logged, seen, a.clone());
    logged.say(seen, "好");
    let seen = logged.ask(2, "第二轮");
    let second = last_turn(&logged);
    let (_, seen) = write(&mut logged, seen, b.clone());
    logged.say(seen, "好");
    assert_eq!(logged.session.todos(), b);
    logged.handle(revert(3, second));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(
        changes(&actions),
        std::slice::from_ref(&a),
        "撤掉第二轮，退回第一轮写的"
    );
    assert_eq!(logged.session.todos(), a);
    logged.handle(unrevert(4));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), std::slice::from_ref(&b), "恢复回来");
}

#[test]
fn a_loaded_session_counts_the_list_back_without_pushing_it() {
    let a = list(&[("一", TodoStatus::InProgress)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    let (_, seen) = write(&mut logged, seen, a.clone());
    logged.say(seen, "好");
    let (session, actions) = load(logged.log.clone());
    assert_eq!(session.todos(), a);
    assert!(changes(&actions).is_empty(), "头订阅时从回应里拿，载入不推");
    // 载入以后接着说一轮：清单没变，照样不推。
    let mut loaded = Logged {
        session,
        log: logged.log.clone(),
    };
    let mut pushed = Vec::new();
    let actions = loaded.handle(send(2, "接着来"));
    pushed.extend(changes(&actions));
    let actions = loaded.handle(stored(loaded.last()));
    pushed.extend(changes(&actions));
    assert!(pushed.is_empty(), "载入时当已经告诉过：{pushed:?}");
}

/// 一步里调了两件：第一件的结果先落了盘，换待办的第二件还没落盘，这时不推；它也落了盘才推（先落盘，后推送）。
#[test]
fn a_list_not_yet_on_disk_is_not_pushed() {
    let a = list(&[("一", TodoStatus::Pending)]);
    let mut logged = Logged::new();
    let seen = logged.ask(1, "开工");
    // 两件只读的一起派出去（工具名不要紧，内核只看效果）。
    let actions = call_tools(&mut logged.session, seen, &[("read", "{}"), ("read", "{}")]);
    logged.log.extend(appended_events(&actions));
    let actions = logged.allowing(stored(logged.last()));
    let ran = ran(&actions);
    logged.handle(done(ran[0], "read"));
    let first = logged.last();
    logged.handle(wrote(ran[1], a.clone()));
    let actions = logged.handle(stored(first));
    assert!(changes(&actions).is_empty(), "换待办的那一条还没落盘");
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(changes(&actions), std::slice::from_ref(&a));
}
