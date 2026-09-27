//! 权限策略（施工 4-3 下）：工具报出要碰的路径，会话照边界和级别判放行、问人还是拒绝。
//!
//! 场地不在系统的临时目录里（[`Home::outside_temp`]）：数据根 `data/`、假的家 `home/`、工作区 `work/`、
//! 边界以外的 `other/`。

mod support;

use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Level, Permission, ToolResult, ToolStatus};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Queued};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use support::*;

/// 会报路径的三件假工具：读（`read`）、写（`edit`）、执行命令（`run`，不报路径）。
struct Kit {
    read: Arc<Fake>,
    edit: Arc<Fake>,
    run: Arc<Fake>,
}

impl Kit {
    fn new() -> Kit {
        Kit {
            read: Fake::new("read", Access::Read, Act::Echo),
            edit: Fake::new("edit", Access::Write, Act::Echo),
            run: Fake::new("run", Access::Execute, Act::Echo),
        }
    }

    fn catalog(&self) -> Catalog {
        Catalog::new(
            [&self.read, &self.edit, &self.run].map(|tool| Arc::clone(tool) as Arc<dyn Tool>),
        )
        .expect("合写法")
    }
}

/// 场地里的一处，写成她会给的样子（绝对路径）。
fn at(home: &Home, path: &str) -> String {
    home.scratch.0.join(path).to_string_lossy().into_owned()
}

/// 调一件工具，参数里是一条路径。
fn one(tool: &str, path: &str) -> Play {
    Play::calls(&[(tool, &serde_json::json!({ "path": path }).to_string())])
}

/// 在工作区 `work/` 里造一个会话：级别 `level`、只读 `read_only`、有没有人能确认 `attended`。
async fn session(
    home: &Home,
    script: &Script,
    kit: &Kit,
    level: Level,
    read_only: bool,
    attended: bool,
) -> Handle {
    let opening = Opening {
        permission: Permission { level, read_only },
        attended,
        cwd: at(home, "work"),
    };
    home.create_as(script, &kit.catalog(), opening).await
}

/// 说一句，等这一轮说完。
async fn turn(handle: &Handle) {
    let mut pushes = watch(handle).await;
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 日志里的第一条工具结果，和记它的是谁。
fn first_result(log: &[Event]) -> (ToolResult, By) {
    log.iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some((result.clone(), event.by.clone())),
            _ => None,
        })
        .expect("有一条工具结果")
}

/// 结果里的字。
fn text(result: &ToolResult) -> String {
    result
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 是权限策略拒绝的。
fn by_permissions(by: &By) -> bool {
    matches!(by, By::Module(module) if module.id.as_str() == "permissions")
}

fn file(home: &Home, path: &str, text: &str) {
    let path = home.scratch.0.join(path);
    std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(path, text).expect("写得进");
}

#[tokio::test]
async fn reads_and_writes_inside_the_workspace_run() {
    let home = Home::outside_temp();
    file(&home, "work/a.txt", "a");
    let kit = Kit::new();
    let script = Script::new([
        one("read", "a.txt"),
        Play::Says("好。"),
        one("edit", "new.txt"),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, &kit, Level::Workspace, false, false).await;
    turn(&handle).await;
    assert_eq!(
        kit.read.calls().len(),
        1,
        "相对路径照这一轮的工作目录接，在工作区里"
    );
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-2", say("再来")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(kit.edit.calls().len(), 1, "工作区里还不存在的文件也能写");
}

#[tokio::test]
async fn a_read_outside_asks_and_the_request_says_where() {
    let home = Home::outside_temp();
    file(&home, "other/b.txt", "b");
    let kit = Kit::new();
    let outside = at(&home, "other/b.txt");
    let script = Script::new([one("read", &outside)]);
    let handle = session(&home, &script, &kit, Level::Workspace, false, true).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    // 等到请求落了盘：她在等人确认。
    let session_id = handle.id().clone();
    until_logged(&home, &session_id, |log| {
        kinds(log).contains(&"tool.approval_requested")
    })
    .await;
    assert!(kit.read.calls().is_empty(), "没跑");
    let log = home.log(&session_id);
    let (request, by) = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ApprovalRequested(request) => Some((request.clone(), event.by.clone())),
            _ => None,
        })
        .expect("有一条确认请求");
    assert!(by_permissions(&by), "{by:?}");
    assert_eq!(request.access, Access::Read);
    let dir = std::fs::canonicalize(home.scratch.0.join("other")).expect("在");
    let rule: serde_json::Value =
        serde_json::from_str(request.rule.expect("提了规则").get()).expect("是 JSON");
    assert_eq!(
        rule,
        serde_json::json!({"tool": "read", "read": [dir.to_string_lossy()]})
    );
    let detail: serde_json::Value =
        serde_json::from_str(request.detail.expect("有说明").get()).expect("是 JSON");
    let real = std::fs::canonicalize(&outside).expect("在");
    assert_eq!(
        detail,
        serde_json::json!({"tool": "read", "paths": [{"path": real.to_string_lossy(), "write": false, "zone": "outside"}]})
    );
    ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

#[tokio::test]
async fn nobody_to_ask_means_no() {
    let home = Home::outside_temp();
    file(&home, "other/b.txt", "b");
    let kit = Kit::new();
    let script = Script::new([one("read", &at(&home, "other/b.txt")), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Workspace, false, false).await;
    turn(&handle).await;
    let (result, _) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Denied);
    assert!(kit.read.calls().is_empty());
}

#[tokio::test]
async fn the_data_root_is_refused_at_every_level() {
    let home = Home::outside_temp();
    let token = home.root.path().join("run").join("token-for-test");
    std::fs::write(&token, "secret").expect("写得进");
    let token = token.to_string_lossy().into_owned();
    for level in [Level::Workspace, Level::Full] {
        let kit = Kit::new();
        let script = Script::new([one("read", &token), Play::Says("好。")]);
        let handle = session(&home, &script, &kit, level.clone(), false, true).await;
        turn(&handle).await;
        let (result, by) = first_result(&home.log(handle.id()));
        assert_eq!(result.status, ToolStatus::Denied, "{level:?}");
        assert!(by_permissions(&by), "{by:?}");
        assert!(
            text(&result).contains("inside Miyu's own data"),
            "{}",
            text(&result)
        );
        assert!(kit.read.calls().is_empty(), "{level:?}");
    }
}

#[tokio::test]
async fn full_access_reads_outside_without_asking() {
    let home = Home::outside_temp();
    file(&home, "other/b.txt", "b");
    let kit = Kit::new();
    let script = Script::new([one("read", &at(&home, "other/b.txt")), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Full, false, true).await;
    turn(&handle).await;
    assert_eq!(kit.read.calls().len(), 1);
}

#[tokio::test]
async fn commands_run_at_the_workspace_level_and_ask_when_read_only() {
    let home = Home::outside_temp();
    let kit = Kit::new();
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Workspace, false, false).await;
    turn(&handle).await;
    assert_eq!(
        kit.run.calls().len(),
        1,
        "M5 之前，工作区这一级执行命令不问"
    );
    // 只读：没有沙盒，每条命令都问人；这个会话没人能确认，就拒绝。
    let kit = Kit::new();
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Workspace, true, false).await;
    turn(&handle).await;
    let (result, _) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Denied);
    assert!(kit.run.calls().is_empty());
}

#[tokio::test]
async fn a_git_hook_is_read_only_in_the_workspace() {
    let home = Home::outside_temp();
    file(&home, "work/.git/hooks/pre-commit", "#!/bin/sh");
    let kit = Kit::new();
    let script = Script::new([one("edit", ".git/hooks/pre-commit"), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Workspace, false, false).await;
    turn(&handle).await;
    let (result, _) = first_result(&home.log(handle.id()));
    assert_eq!(
        result.status,
        ToolStatus::Denied,
        "要问人，这个会话没人能确认"
    );
    assert!(kit.edit.calls().is_empty());
}

#[tokio::test]
async fn several_paths_go_by_the_strictest() {
    let home = Home::outside_temp();
    file(&home, "work/a.txt", "a");
    file(&home, "other/b.txt", "b");
    let token = home.root.path().join("run").join("token-for-test");
    std::fs::write(&token, "secret").expect("写得进");
    let kit = Kit::new();
    let args = serde_json::json!({ "paths": [at(&home, "work/a.txt"), token.to_string_lossy(), at(&home, "other/b.txt")] }).to_string();
    let script = Script::new([Play::calls(&[("read", &args)]), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Workspace, false, true).await;
    turn(&handle).await;
    let (result, by) = first_result(&home.log(handle.id()));
    assert_eq!(
        result.status,
        ToolStatus::Denied,
        "有一条在数据根里：拒绝，不问人"
    );
    assert!(by_permissions(&by));
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_to_nowhere_is_refused_and_says_why() {
    let home = Home::outside_temp();
    std::os::unix::fs::symlink(
        home.scratch.0.join("nowhere"),
        home.scratch.0.join("work/dead"),
    )
    .expect("造得了链接");
    let kit = Kit::new();
    let script = Script::new([one("read", "dead/x.txt"), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Workspace, false, true).await;
    turn(&handle).await;
    let (result, by) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Denied);
    assert!(by_permissions(&by));
    assert_eq!(
        text(&result),
        "Can't tell where \"dead/x.txt\" points: a link on the path points nowhere.\n"
    );
}

#[tokio::test]
async fn a_write_to_a_git_hook_asks_and_says_it_is_read_only() {
    let home = Home::outside_temp();
    file(&home, "work/.git/hooks/pre-commit", "#!/bin/sh");
    let kit = Kit::new();
    let script = Script::new([one("edit", ".git/hooks/pre-commit")]);
    let handle = session(&home, &script, &kit, Level::Workspace, false, true).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let session_id = handle.id().clone();
    let log = until_logged(&home, &session_id, |log| {
        kinds(log).contains(&"tool.approval_requested")
    })
    .await;
    let request = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ApprovalRequested(request) => Some(request.clone()),
            _ => None,
        })
        .expect("有一条确认请求");
    assert_eq!(request.access, Access::Write);
    let detail: serde_json::Value =
        serde_json::from_str(request.detail.expect("有说明").get()).expect("是 JSON");
    assert_eq!(detail["paths"][0]["zone"], "read_only");
    assert_eq!(detail["paths"][0]["write"], true);
    let rule: serde_json::Value =
        serde_json::from_str(request.rule.expect("提了规则").get()).expect("是 JSON");
    assert!(
        rule.get("write").is_some() && rule.get("read").is_none(),
        "{rule}"
    );
    ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

#[tokio::test]
async fn a_tilde_path_follows_the_home() {
    let home = Home::outside_temp();
    file(&home, "home/notes.txt", "notes");
    let kit = Kit::new();
    let script = Script::new([one("read", "~/notes.txt"), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Workspace, false, false).await;
    turn(&handle).await;
    let (result, by) = first_result(&home.log(handle.id()));
    // 换得成真实的位置（家目录下），在边界以外，要问人；没人能确认，内核拒了。说不清在哪的才是权限策略拒的。
    assert_eq!(result.status, ToolStatus::Denied);
    assert!(!by_permissions(&by), "{by:?}：{}", text(&result));
    assert!(!text(&result).contains("Can't tell"), "{}", text(&result));
}
