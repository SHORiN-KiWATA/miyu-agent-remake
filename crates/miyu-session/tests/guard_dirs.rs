//! 加进来的目录（施工 5-10 上，`docs/blueprint/session/guard.md` 边界表第 3 条）：权限策略照工作区算，里面的
//! `.git/hooks` 只能读，只读照旧拒绝。场地和 `guard.rs` 一样不在系统的临时目录里：工作区 `work/`、加进来的
//! `other/`。

use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Level, Permission, ToolResult, ToolStatus};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 写文件的假工具，报它参数里的路径。
struct Kit {
    edit: Arc<Fake>,
}

impl Kit {
    fn new() -> Kit {
        Kit {
            edit: Fake::new("edit", Access::Write, Act::Echo),
        }
    }

    fn catalog(&self) -> Catalog {
        Catalog::new([Arc::clone(&self.edit) as Arc<dyn Tool>]).expect("合写法")
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

fn file(home: &Home, path: &str, text: &str) {
    let path = home.scratch.0.join(path);
    std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(path, text).expect("写得进");
}

/// 在工作区 `work/` 里造一个会话，加进来的目录是场地里的 `dirs`：工作区这一级、只读 `read_only`，没人能确认，沙盒
/// 用不了（施工 5-10 上）。
async fn with_dirs(
    home: &Home,
    script: &Script,
    kit: &Kit,
    read_only: bool,
    dirs: &[&str],
) -> Handle {
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only,
        },
        attended: false,
        cwd: at(home, "work"),
        dirs: dirs.iter().map(|dir| at(home, dir)).collect(),
        sandbox: None,
        sandbox_cache: None,
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
fn first_result(log: &[Event]) -> (ToolResult, miyu_kernel::origin::By) {
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

/// 加进来的目录（施工 5-10 上）：工作区这一级和工作区一样能写，不用问人。
#[tokio::test]
async fn a_write_into_an_added_dir_runs_without_asking() {
    let home = Home::outside_temp();
    file(&home, "other/b.txt", "b");
    let kit = Kit::new();
    let script = Script::new([one("edit", &at(&home, "other/b.txt")), Play::Says("好。")]);
    let handle = with_dirs(&home, &script, &kit, false, &["other"]).await;
    turn(&handle).await;
    let (result, _) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Ok, "{}", text(&result));
    assert_eq!(kit.edit.calls().len(), 1);
}

#[tokio::test]
async fn a_git_hook_in_an_added_dir_is_read_only() {
    let home = Home::outside_temp();
    file(&home, "other/.git/hooks/pre-commit", "#!/bin/sh");
    let kit = Kit::new();
    let hook = at(&home, "other/.git/hooks/pre-commit");
    let script = Script::new([one("edit", &hook), Play::Says("好。")]);
    let handle = with_dirs(&home, &script, &kit, false, &["other"]).await;
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
async fn read_only_still_refuses_a_write_into_an_added_dir() {
    let home = Home::outside_temp();
    file(&home, "other/b.txt", "b");
    let kit = Kit::new();
    let script = Script::new([one("edit", &at(&home, "other/b.txt")), Play::Says("好。")]);
    let handle = with_dirs(&home, &script, &kit, true, &["other"]).await;
    turn(&handle).await;
    let (result, _) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Denied);
    assert!(kit.edit.calls().is_empty());
}
