//! 撤销、恢复时改回文件（`docs/designs/10-自带软件.md` 第七节「改回文件的细则」，施工 4-7 上）：撤掉的那一轮改过
//! 文件的，同一批交出改回文件，改的时候不接命令、不算空闲；结局回来记一条 `files.restored`，两条都落了盘才回应；
//! 恢复照同样的办法改回撤销前的样子；没改过文件的照旧当场回应；过时的结局不理。

use super::executor::*;
use super::load::Logged;
use super::revert::{revert, unrevert};
use super::*;
use crate::event::{Effect, FileChanged, FilesRestored, RestoreOutcome};
use crate::id::{CallId, ContentHash};
use crate::session::{Expect, Step, StepAction};

/// 一段内容的哈希。
fn hash(text: &str) -> ContentHash {
    ContentHash::of(text.as_bytes())
}

/// 调用 `call_id` 执行完了，把 `/w/a.txt` 从 A 改成了 B。
pub(super) fn changed(call_id: CallId) -> Input {
    Input::ToolDone {
        at: at(50),
        call_id,
        error: false,
        blocks: Vec::new(),
        duration_ms: Some(3),
        human: None,
        effects: vec![Effect::FileChanged(FileChanged {
            path: "/w/a.txt".to_string(),
            before: Some(hash("A")),
            after: hash("B"),
        })],
        stopped: false,
    }
}

/// 一轮（3 号）：她调了一次 `write`，把 `/w/a.txt` 从 A 改成了 B，说完了，全落了盘。交回工具结果的序号。
pub(super) fn edited() -> (Logged, Seq) {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "改一下");
    logged.tools(seen, &[("write", r#"{"file_path":"a.txt"}"#)]);
    let reply = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::MessageAssistant(_)))
        .map(|event| event.seq)
        .unwrap();
    logged.handle(changed(call(reply.get(), 1)));
    let result = logged
        .log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::ToolResult(_)))
        .map(|event| event.seq)
        .unwrap();
    let actions = logged.handle(stored(logged.last()));
    let (seen, _) = calls(&actions).remove(0);
    logged.say(seen.get(), "改好了");
    (logged, result)
}

/// 这一批交出的改回文件。
pub(super) fn steps_of(actions: &[Action]) -> Vec<Step> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Restore { steps } => Some(steps.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// 这几步都改回了的结局。
fn done(steps: &[Step]) -> Input {
    Input::Restored {
        at: at(58),
        files: steps.iter().map(crate::testkit::restored).collect(),
    }
}

#[test]
fn an_undo_that_changed_files_waits_for_them_before_replying() {
    let (mut logged, result) = edited();
    let actions = logged.handle(revert(9, 3));
    let reverted = logged.last();
    assert_eq!(appended(&actions), seqs(&[reverted]), "只记了撤销");
    let steps = steps_of(&actions);
    assert_eq!(
        steps,
        [Step {
            result,
            effect: 0,
            path: "/w/a.txt".to_string(),
            action: StepAction::Write {
                expect: Expect::Content(hash("B")),
                content: hash("A"),
            },
        }]
    );
    assert!(replies(&actions).is_empty(), "改完文件才回应");
    let actions = logged.handle(stored(reverted));
    assert!(replies(&actions).is_empty(), "撤销落了盘也还不回应");
    assert!(!logged.session.idle(), "改回文件的时候不算空闲");
    // 改回文件的时候来的命令：拒绝，手动压缩也一样（施工 6-8）。
    for command in [send(10, "hi"), super::compact::compact(11, None)] {
        let actions = logged.handle(command);
        assert!(
            matches!(
                actions.as_slice(),
                [Action::Reply {
                    outcome: Outcome::Rejected {
                        reason: Reason::Restoring,
                        ..
                    },
                    ..
                }]
            ),
            "{actions:?}"
        );
    }
    let actions = logged.handle(done(&steps));
    let events = appended_events(&actions);
    assert_eq!(events.len(), 1);
    let files = steps.iter().map(crate::testkit::restored).collect();
    assert_eq!(events[0].body, Body::FilesRestored(FilesRestored { files }));
    assert_eq!(
        (&events[0].by, &events[0].cause, events[0].turn),
        (&alice(), &Some(id(9)), None)
    );
    let restored = events[0].seq.get();
    let actions = logged.handle(stored(restored));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(9, &[reverted, restored])]
    );
    assert!(logged.session.idle());
}

#[test]
fn a_redo_puts_the_files_back_the_same_way() {
    let (mut logged, result) = edited();
    let actions = logged.handle(revert(9, 3));
    logged.handle(stored(logged.last()));
    logged.handle(done(&steps_of(&actions)));
    logged.handle(stored(logged.last()));
    let actions = logged.handle(unrevert(10));
    let unreverted = logged.last();
    assert_eq!(
        steps_of(&actions),
        [Step {
            result,
            effect: 0,
            path: "/w/a.txt".to_string(),
            action: StepAction::Write {
                expect: Expect::Content(hash("A")),
                content: hash("B"),
            },
        }]
    );
    logged.handle(stored(unreverted));
    logged.handle(done(&steps_of(&actions)));
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(10, &[unreverted, logged.last()])]
    );
}

#[test]
fn an_undo_without_file_changes_replies_as_before() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.say(seen, "好");
    let actions = logged.handle(revert(3, 3));
    assert!(steps_of(&actions).is_empty(), "没改过文件，不交出改回文件");
    let actions = logged.handle(stored(logged.last()));
    assert_eq!(replies(&actions), [&accepted_reply(3, &[logged.last()])]);
    // 没在改的时候来的结局是过时的：不理。
    assert!(logged.handle(done(&[])).is_empty());
}

#[test]
fn a_report_that_misses_a_step_records_it_as_not_done() {
    let (mut logged, _) = edited();
    let actions = logged.handle(revert(9, 3));
    let steps = steps_of(&actions);
    logged.handle(stored(logged.last()));
    // 执行器交回的一项都没有：照那一步补一项出错，下一次恢复照它当没改回。
    let actions = logged.handle(Input::Restored {
        at: at(58),
        files: Vec::new(),
    });
    let events = appended_events(&actions);
    let Body::FilesRestored(FilesRestored { files }) = &events[0].body else {
        panic!("{events:?}");
    };
    assert_eq!(files.len(), 1);
    assert_eq!(
        (files[0].result, &files[0].path, &files[0].outcome),
        (steps[0].result, &steps[0].path, &RestoreOutcome::Failed)
    );
    assert_eq!(
        files[0].error.as_deref(),
        Some("executor report did not match")
    );
}
