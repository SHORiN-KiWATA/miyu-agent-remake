//! 撤销、恢复要改回的几步：撤销倒着排，改过的写回改前的，新建的移进回收站，删掉的移回来，读过的不算；恢复正着排，
//! 新建的要原处空着，没移回来的不再移进回收站；来回几次以后，照最新的位置。

use super::*;

/// 一段内容的哈希。
fn hash(text: &str) -> ContentHash {
    ContentHash::of(text.as_bytes())
}

/// 哈希在 JSON 里的写法。
fn json(text: &str) -> String {
    serde_json::to_string(&hash(text)).expect("写得出去")
}

/// 第 `seq` 条：一条工具结果，效果是 `effects`（一段 JSON 数组）。
fn result(seq: u64, effects: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-09-28T00:00:00.000Z","kind":"tool.result","turn":2,"by":{{"kind":"tool","call_id":"call_1_1"}},"body":{{"call_id":"call_1_1","status":"ok","blocks":[],"effects":{effects}}}}}"#
    ))
    .expect("读得出来")
}

/// 第 `seq` 条：一条 `files.restored`，每一步是 `files`（一段 JSON 数组）。
fn restored(seq: u64, files: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-09-28T00:00:00.000Z","kind":"files.restored","by":{{"kind":"person","account":"admin"}},"body":{{"files":{files}}}}}"#
    ))
    .expect("读得出来")
}

/// 一轮里：读了 `x`、改了 `a`（第 10 条），新建了 `n`（第 12 条），删了 `t`（第 14 条）。
fn turn() -> Vec<Event> {
    vec![
        result(
            10,
            &format!(
                r#"[{{"kind":"file.read","path":"/w/x","hash":{}}},{{"kind":"file.changed","path":"/w/a","before":{},"after":{}}}]"#,
                json("x"),
                json("A"),
                json("B")
            ),
        ),
        result(
            12,
            &format!(
                r#"[{{"kind":"file.changed","path":"/w/n","before":null,"after":{}}}]"#,
                json("N")
            ),
        ),
        result(
            14,
            r#"[{"kind":"file.trashed","path":"/w/t","trash":"/T/t"}]"#,
        ),
    ]
}

/// 第 `result` 条的第 0 个效果、在 `path` 上的一步。
fn step(result: u64, effect: u32, path: &str, action: StepAction) -> Step {
    Step {
        result: Seq::new(result).expect("不是 0"),
        effect,
        path: path.to_string(),
        action,
    }
}

#[test]
fn undo_goes_backwards_and_skips_what_was_only_read() {
    assert_eq!(
        undo(&turn(), &[]),
        [
            step(
                14,
                0,
                "/w/t",
                StepAction::Untrash {
                    from: "/T/t".into()
                }
            ),
            step(
                12,
                0,
                "/w/n",
                StepAction::Trash {
                    expect: Expect::Content(hash("N"))
                }
            ),
            step(
                10,
                1,
                "/w/a",
                StepAction::Write {
                    expect: Expect::Content(hash("B")),
                    content: hash("A"),
                }
            ),
        ]
    );
}

#[test]
fn redo_goes_forwards_and_puts_back_only_what_came_back() {
    let back = restored(
        20,
        &format!(
            r#"[{{"result":14,"effect":0,"path":"/w/t","action":"untrash","outcome":"restored","hash":{}}}]"#,
            json("T")
        ),
    );
    assert_eq!(
        redo(&turn(), &[back]),
        [
            step(
                10,
                1,
                "/w/a",
                StepAction::Write {
                    expect: Expect::Content(hash("A")),
                    content: hash("B"),
                }
            ),
            step(
                12,
                0,
                "/w/n",
                StepAction::Write {
                    expect: Expect::Absent,
                    content: hash("N"),
                }
            ),
            step(
                14,
                0,
                "/w/t",
                StepAction::Trash {
                    expect: Expect::Content(hash("T"))
                }
            ),
        ]
    );
    // 上一次撤销没移回来（原处被占了）：没有要再移进回收站的。
    let occupied = restored(
        20,
        r#"[{"result":14,"effect":0,"path":"/w/t","action":"untrash","outcome":"occupied"}]"#,
    );
    let steps = redo(&turn(), &[occupied]);
    assert_eq!(steps.len(), 2, "{steps:?}");
    // 移回来的是目录，没记哈希：有东西就行。
    let dir = restored(
        20,
        r#"[{"result":14,"effect":0,"path":"/w/t","action":"untrash","outcome":"restored"}]"#,
    );
    assert_eq!(
        redo(&turn(), &[dir]).last().map(|step| step.action.clone()),
        Some(StepAction::Trash {
            expect: Expect::Present
        })
    );
}

#[test]
fn after_a_round_trip_the_newest_place_is_used() {
    let back = restored(
        20,
        r#"[{"result":14,"effect":0,"path":"/w/t","action":"untrash","outcome":"restored"}]"#,
    );
    let again = restored(
        22,
        r#"[{"result":14,"effect":0,"path":"/w/t","action":"trash","outcome":"restored","trash":"/T/t.2"}]"#,
    );
    assert_eq!(
        undo(&turn(), &[back.clone(), again])[0].action,
        StepAction::Untrash {
            from: "/T/t.2".into()
        }
    );
    // 恢复时没移进去（回收站收不了）：它还在原处，再撤销没有要移回来的。
    let stayed = restored(
        22,
        r#"[{"result":14,"effect":0,"path":"/w/t","action":"trash","outcome":"unavailable"}]"#,
    );
    let steps = undo(&turn(), &[back, stayed]);
    assert!(steps.iter().all(|step| step.path != "/w/t"), "{steps:?}");
}

#[test]
fn a_step_done_is_reported_with_its_action() {
    let steps = undo(&turn(), &[]);
    let done: Vec<(RestoreAction, RestoreOutcome)> = steps
        .iter()
        .map(|step| {
            let restored = step.restored();
            (restored.action, restored.outcome)
        })
        .collect();
    assert_eq!(
        done,
        [
            (RestoreAction::Untrash, RestoreOutcome::Restored),
            (RestoreAction::Trash, RestoreOutcome::Restored),
            (RestoreAction::Write, RestoreOutcome::Restored),
        ]
    );
    assert_eq!(steps[2].restored().result.get(), 10);
    assert_eq!(steps[2].restored().effect, 1);
}
