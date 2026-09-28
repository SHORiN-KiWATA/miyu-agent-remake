//! 撤销、恢复时改回文件（施工 4-7 上），会话里用真的工具：改过的写回去，恢复时再改回来；之后被人改过的不动，记下
//! 现在的哈希；她看过的照有效历史走。新建的、删掉的要进出回收站，回收站在场地的假家目录里，只在 Linux 上跑：别的
//! 平台进的是系统真的回收站。

mod support;

use std::path::Path;

use miyu_kernel::event::{
    Body, Level, Permission, RestoreAction, RestoreOutcome, Restored, Said, ToolResult,
};
use miyu_kernel::id::{ContentHash, TurnId};
use miyu_kernel::session::{Command, Outcome};
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

use support::*;

fn base_system() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 在场地的 `work/` 里干活，工作区这一级，没人能确认。
fn opening(home: &Home) -> Opening {
    Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    }
}

/// 说一句，等这一轮结束。
async fn talk(handle: &miyu_session::Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 日志里最后一轮的编号。
fn last_turn(home: &Home, handle: &miyu_session::Handle) -> TurnId {
    home.log(handle.id())
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| TurnId::new(event.seq))
        .expect("开过一轮")
}

/// 日志里最后一条 `files.restored` 的每一步。
fn last_restored(home: &Home, handle: &miyu_session::Handle) -> Vec<Restored> {
    home.log(handle.id())
        .into_iter()
        .rev()
        .find_map(|event| match event.body {
            Body::FilesRestored(restored) => Some(restored.files),
            _ => None,
        })
        .expect("记了改回文件的结局")
}

/// 日志里的工具结果。
fn results(home: &Home, handle: &miyu_session::Handle) -> Vec<ToolResult> {
    home.log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

/// 场地 `work/` 里的一个文件的内容；没有的是空的。
fn read(home: &Home, name: &str) -> Option<String> {
    std::fs::read_to_string(home.scratch.0.join("work").join(name)).ok()
}

/// 在场地的 `work/` 里写一个文件，像是别人写的。
fn put(home: &Home, name: &str, text: &str) {
    std::fs::write(home.scratch.0.join("work").join(name), text).expect("写得进");
}

/// 她照 `plays` 走一轮：造会话，说一句，等这一轮结束。
async fn one_turn(home: &Home, plays: Vec<Play>) -> miyu_session::Handle {
    let script = Script::new(plays);
    let handle = home.create_as(&script, &base_system(), opening(home)).await;
    talk(&handle, "cmd-1", "改一下").await;
    handle
}

/// 读 `a.txt`，再把它写成 `new\n`。
fn edit_a() -> Vec<Play> {
    vec![
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("write", r#"{"file_path":"a.txt","content":"new\n"}"#)]),
        Play::Says("改好了。"),
    ]
}

/// 发撤销或者恢复，要接受，产生两条事件：撤销（恢复）和改回文件的结局。
async fn undo(handle: &miyu_session::Handle, id: &str, command: Command) {
    match ask(handle, id, command).await.expect("会话在跑") {
        Outcome::Accepted { events } => assert_eq!(events.len(), 2, "{events:?}"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn an_edit_is_written_back_and_redone() {
    let home = Home::outside_temp();
    let file = home.scratch.0.join("work/a.txt");
    std::fs::write(&file, "old\n").expect("写得进");
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("write", r#"{"file_path":"a.txt","content":"new\n"}"#)]),
        Play::Says("改好了。"),
    ]);
    let handle = home
        .create_as(&script, &base_system(), opening(&home))
        .await;
    talk(&handle, "cmd-1", "改一下").await;
    assert_eq!(std::fs::read(&file).expect("读得出"), b"new\n");
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    assert_eq!(std::fs::read(&file).expect("读得出"), b"old\n", "写回去了");
    let steps = last_restored(&home, &handle);
    assert_eq!(steps.len(), 1, "读过的不用改回：{steps:?}");
    assert_eq!(
        (&steps[0].action, &steps[0].outcome),
        (&RestoreAction::Write, &RestoreOutcome::Restored)
    );
    undo(&handle, "cmd-3", Command::Unrevert).await;
    assert_eq!(
        std::fs::read(&file).expect("读得出"),
        b"new\n",
        "又改回来了"
    );
}

#[tokio::test]
async fn what_someone_changed_since_is_left_alone() {
    let home = Home::outside_temp();
    let file = home.scratch.0.join("work/a.txt");
    std::fs::write(&file, "old\n").expect("写得进");
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("write", r#"{"file_path":"a.txt","content":"new\n"}"#)]),
        Play::Says("改好了。"),
    ]);
    let handle = home
        .create_as(&script, &base_system(), opening(&home))
        .await;
    talk(&handle, "cmd-1", "改一下").await;
    // 她改完以后，有人又改了一次。
    std::fs::write(&file, "someone\n").expect("写得进");
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    assert_eq!(std::fs::read(&file).expect("读得出"), b"someone\n", "不动");
    let steps = last_restored(&home, &handle);
    assert_eq!(steps[0].outcome, RestoreOutcome::Changed);
    assert_eq!(steps[0].found, Some(ContentHash::of(b"someone\n")));
}

/// 撤掉的那一轮里读过的不算她看过：原处写回了她没看过的样子，接着改要先读。
#[tokio::test]
async fn what_she_read_in_an_undone_turn_no_longer_counts() {
    let home = Home::outside_temp();
    let file = home.scratch.0.join("work/a.txt");
    std::fs::write(&file, "old\n").expect("写得进");
    let write = r#"{"file_path":"a.txt","content":"new\n"}"#;
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("write", write)]),
        Play::Says("改好了。"),
        Play::calls(&[("write", write)]),
        Play::Says("好。"),
    ]);
    let handle = home
        .create_as(&script, &base_system(), opening(&home))
        .await;
    talk(&handle, "cmd-1", "改一下").await;
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    talk(&handle, "cmd-3", "再改一下").await;
    let last = results(&home, &handle).pop().expect("有结果");
    assert_eq!(
        last.human,
        Some(Said::new("software/basesystem/common/not-read"))
    );
    assert_eq!(std::fs::read(&file).expect("读得出"), b"old\n", "没写");
}

/// 现在已经是要改成的样子（有人先改回去了）：算改回了，不当冲突。
#[tokio::test]
async fn what_is_already_back_counts_as_restored() {
    let home = Home::outside_temp();
    put(&home, "a.txt", "old\n");
    let handle = one_turn(&home, edit_a()).await;
    put(&home, "a.txt", "old\n");
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    assert_eq!(
        last_restored(&home, &handle)[0].outcome,
        RestoreOutcome::Restored
    );
    assert_eq!(read(&home, "a.txt").as_deref(), Some("old\n"));
}

/// 改前的内容当时没存下来（这里删掉那份 blob）：记 `unsaved`，文件不动。
#[tokio::test]
async fn content_that_was_not_saved_is_reported() {
    let home = Home::outside_temp();
    put(&home, "a.txt", "old\n");
    let handle = one_turn(&home, edit_a()).await;
    let blobs = Blobs::new(home.root.blobs(&alice_account()));
    std::fs::remove_file(blobs.path(&ContentHash::of(b"old\n"))).expect("删得掉");
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    assert_eq!(
        last_restored(&home, &handle)[0].outcome,
        RestoreOutcome::Unsaved
    );
    assert_eq!(read(&home, "a.txt").as_deref(), Some("new\n"));
}

/// 恢复以后，那一轮里读过的又算她看过：接着改用不着重读。
#[tokio::test]
async fn after_a_redo_what_she_read_counts_again() {
    let home = Home::outside_temp();
    put(&home, "a.txt", "old\n");
    let mut plays = edit_a();
    plays.push(Play::calls(&[(
        "write",
        r#"{"file_path":"a.txt","content":"newer\n"}"#,
    )]));
    plays.push(Play::Says("好。"));
    let handle = one_turn(&home, plays).await;
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    undo(&handle, "cmd-3", Command::Unrevert).await;
    talk(&handle, "cmd-4", "再改一下").await;
    assert_eq!(read(&home, "a.txt").as_deref(), Some("newer\n"));
}

#[cfg(target_os = "linux")]
#[path = "restore/linux.rs"]
mod linux;
