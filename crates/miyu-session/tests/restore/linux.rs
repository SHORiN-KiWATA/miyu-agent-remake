//! 进出回收站的那几种（Linux，回收站在场地的假家目录里）：新建的、改过的、删掉的一轮里都有，撤销以后都回到原样，
//! 恢复以后又回到她改完的样子；恢复时位置变了的，再撤销照新的位置；被人动过的、回收站里没了的、收不了的都不动，
//! 记下是哪一种；恢复时上级目录没了的建上。

use super::*;

/// 场地里假家目录的回收站。
fn bin(home: &Home) -> std::path::PathBuf {
    home.scratch.0.join("home/.local/share/Trash")
}

/// 一轮里新建一个、改一个、删一个。交回会话。
async fn three_changes(home: &Home) -> miyu_session::Handle {
    for (name, text) in [("a.txt", "old\n"), ("t.txt", "bye\n")] {
        std::fs::write(home.scratch.0.join("work").join(name), text).expect("写得进");
    }
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[
            ("write", r#"{"file_path":"a.txt","content":"new\n"}"#),
            ("write", r#"{"file_path":"n.txt","content":"made\n"}"#),
            ("trash", r#"{"file_path":"t.txt"}"#),
        ]),
        Play::Says("改好了。"),
    ]);
    let handle = home.create_as(&script, &base_system(), opening(home)).await;
    talk(&handle, "cmd-1", "改一下").await;
    handle
}

#[tokio::test]
async fn created_changed_and_trashed_files_all_come_back() {
    let home = Home::outside_temp();
    let handle = three_changes(&home).await;
    assert_eq!(read(&home, "t.txt"), None, "删进了回收站");
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn }).await;
    assert_eq!(read(&home, "a.txt").as_deref(), Some("old\n"));
    assert_eq!(read(&home, "n.txt"), None, "新建的移进了回收站");
    assert_eq!(read(&home, "t.txt").as_deref(), Some("bye\n"), "移回来了");
    assert!(
        !bin(&home).join("info/t.txt.trashinfo").exists(),
        "记录删了"
    );
    assert_eq!(
        std::fs::read_to_string(bin(&home).join("files/n.txt")).expect("在回收站里"),
        "made\n"
    );
    let steps = last_restored(&home, &handle);
    let done: Vec<(&RestoreAction, &RestoreOutcome)> = steps
        .iter()
        .map(|step| (&step.action, &step.outcome))
        .collect();
    assert_eq!(
        done,
        [
            (&RestoreAction::Untrash, &RestoreOutcome::Restored),
            (&RestoreAction::Trash, &RestoreOutcome::Restored),
            (&RestoreAction::Write, &RestoreOutcome::Restored),
        ],
        "倒着改回"
    );
    undo(&handle, "cmd-3", Command::Unrevert).await;
    assert_eq!(read(&home, "a.txt").as_deref(), Some("new\n"));
    assert_eq!(read(&home, "n.txt").as_deref(), Some("made\n"));
    assert_eq!(read(&home, "t.txt"), None, "又进了回收站");
}

/// 恢复时再移进回收站，那里已经有一个同名的，位置变成 `t.txt.2`：再撤销照新的位置移回来，同名的那个不动。
#[tokio::test]
async fn undo_after_redo_uses_the_new_place_in_the_trash() {
    let home = Home::outside_temp();
    let handle = three_changes(&home).await;
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn }).await;
    std::fs::write(bin(&home).join("files/t.txt"), "someone's\n").expect("写得进");
    undo(&handle, "cmd-3", Command::Unrevert).await;
    let moved = last_restored(&home, &handle)
        .into_iter()
        .find(|step| step.action == RestoreAction::Trash)
        .and_then(|step| step.trash)
        .expect("记下了新的位置");
    assert!(moved.ends_with("files/t.txt.2"), "{moved}");
    undo(&handle, "cmd-4", Command::Revert { turn }).await;
    assert_eq!(read(&home, "t.txt").as_deref(), Some("bye\n"));
    assert_eq!(
        std::fs::read_to_string(bin(&home).join("files/t.txt")).expect("还在"),
        "someone's\n"
    );
}

/// 新建一个文件的一轮。
fn create(path: &str) -> Vec<Play> {
    vec![
        Play::calls(&[(
            "write",
            &*format!(r#"{{"file_path":"{path}","content":"made\n"}}"#),
        )]),
        Play::Says("好了。"),
    ]
}

/// 删掉 `t.txt` 的一轮。
fn trash_t() -> Vec<Play> {
    vec![
        Play::calls(&[("trash", r#"{"file_path":"t.txt"}"#)]),
        Play::Says("删了。"),
    ]
}

/// 撤掉最后一轮，交回这一次改回文件的每一步的结局。
async fn undo_last(home: &Home, handle: &miyu_session::Handle, id: &str) -> Vec<RestoreOutcome> {
    let turn = last_turn(home, handle);
    undo(handle, id, Command::Revert { turn }).await;
    outcomes(home, handle)
}

/// 最后一次改回文件的每一步的结局。
fn outcomes(home: &Home, handle: &miyu_session::Handle) -> Vec<RestoreOutcome> {
    last_restored(home, handle)
        .into_iter()
        .map(|step| step.outcome)
        .collect()
}

#[tokio::test]
async fn a_created_file_someone_changed_stays() {
    let home = Home::outside_temp();
    let handle = one_turn(&home, create("n.txt")).await;
    put(&home, "n.txt", "someone\n");
    assert_eq!(
        undo_last(&home, &handle, "cmd-2").await,
        [RestoreOutcome::Changed]
    );
    assert_eq!(read(&home, "n.txt").as_deref(), Some("someone\n"), "不动");
}

#[tokio::test]
async fn a_trashed_file_whose_place_is_taken_stays_in_the_trash() {
    let home = Home::outside_temp();
    put(&home, "t.txt", "bye\n");
    let handle = one_turn(&home, trash_t()).await;
    put(&home, "t.txt", "new one\n");
    assert_eq!(
        undo_last(&home, &handle, "cmd-2").await,
        [RestoreOutcome::Occupied]
    );
    assert_eq!(read(&home, "t.txt").as_deref(), Some("new one\n"), "不盖掉");
    assert!(bin(&home).join("files/t.txt").is_file(), "还在回收站里");
}

#[tokio::test]
async fn a_file_no_longer_in_the_trash_is_reported_gone() {
    let home = Home::outside_temp();
    put(&home, "t.txt", "bye\n");
    let handle = one_turn(&home, trash_t()).await;
    std::fs::remove_file(bin(&home).join("files/t.txt")).expect("删得掉");
    assert_eq!(
        undo_last(&home, &handle, "cmd-2").await,
        [RestoreOutcome::Gone]
    );
    assert_eq!(read(&home, "t.txt"), None);
}

/// 撤销时移回来以后又被人改过：恢复时不再移进回收站。
#[tokio::test]
async fn a_file_changed_after_coming_back_is_not_trashed_again() {
    let home = Home::outside_temp();
    put(&home, "t.txt", "bye\n");
    let handle = one_turn(&home, trash_t()).await;
    undo_last(&home, &handle, "cmd-2").await;
    put(&home, "t.txt", "changed\n");
    undo(&handle, "cmd-3", Command::Unrevert).await;
    assert_eq!(outcomes(&home, &handle), [RestoreOutcome::Changed]);
    assert_eq!(read(&home, "t.txt").as_deref(), Some("changed\n"));
}

/// 家目录的回收站建不起来（被一个文件占了）：新建的那个不删，记 `unavailable`。
#[tokio::test]
async fn when_the_trash_cannot_take_it_nothing_is_deleted() {
    let home = Home::outside_temp();
    let handle = one_turn(&home, create("n.txt")).await;
    std::fs::write(home.scratch.0.join("home/.local"), "not a directory").expect("写得进");
    assert_eq!(
        undo_last(&home, &handle, "cmd-2").await,
        [RestoreOutcome::Unavailable]
    );
    assert_eq!(read(&home, "n.txt").as_deref(), Some("made\n"));
}

/// 撤销以后有人在原处放了一个同名的：恢复时不盖掉它。
#[tokio::test]
async fn a_created_file_is_not_redone_over_someone_elses() {
    let home = Home::outside_temp();
    let handle = one_turn(&home, create("n.txt")).await;
    undo_last(&home, &handle, "cmd-2").await;
    put(&home, "n.txt", "theirs\n");
    undo(&handle, "cmd-3", Command::Unrevert).await;
    assert_eq!(outcomes(&home, &handle), [RestoreOutcome::Occupied]);
    assert_eq!(read(&home, "n.txt").as_deref(), Some("theirs\n"));
}

/// 撤销以后连目录一起没了：恢复时照样建上目录再写。
#[tokio::test]
async fn a_redo_rebuilds_the_directories_it_needs() {
    let home = Home::outside_temp();
    let handle = one_turn(&home, create("sub/n.txt")).await;
    undo_last(&home, &handle, "cmd-2").await;
    std::fs::remove_dir(home.scratch.0.join("work/sub")).expect("空了删得掉");
    undo(&handle, "cmd-3", Command::Unrevert).await;
    assert_eq!(outcomes(&home, &handle), [RestoreOutcome::Restored]);
    assert_eq!(read(&home, "sub/n.txt").as_deref(), Some("made\n"));
}
