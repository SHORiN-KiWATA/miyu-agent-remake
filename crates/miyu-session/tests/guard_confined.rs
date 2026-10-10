//! 只碰得到自己工作区的会话（施工 5-12，`11-权限与沙盒.md` 第三节「外部身份」）：场所会话、属主不是管理员的，文件类工具只放行
//! 工作区里的，外面的（连读）一律拒，写给她的是出厂的那一句，说法 `core/permissions/outside-workspace`；命令一律拒（沙盒把读
//! 也关进工作区以后再放开）。管理员本人的场所会话、本机的会话照旧。场地不在系统的临时目录里：工作区 `work/`。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Level, Permission, ToolResult, ToolStatus};
use miyu_kernel::id::VenueId;
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 读、写、跑命令的三件假工具，报参数里的路径。
struct Kit {
    read: Arc<Fake>,
    write: Arc<Fake>,
    shell: Arc<Fake>,
}

impl Kit {
    fn new() -> Kit {
        Kit {
            read: Fake::new("read", Access::Read, Act::Echo),
            write: Fake::new("write", Access::Write, Act::Echo),
            shell: Fake::new("shell", Access::Execute, Act::Echo),
        }
    }

    fn catalog(&self) -> Catalog {
        Catalog::new([
            Arc::clone(&self.read) as Arc<dyn Tool>,
            Arc::clone(&self.write) as Arc<dyn Tool>,
            Arc::clone(&self.shell) as Arc<dyn Tool>,
        ])
        .expect("合写法")
    }
}

/// 场地里的一处，写成绝对路径。
fn at(home: &Home, path: &str) -> String {
    home.scratch.0.join(path).to_string_lossy().into_owned()
}

/// 调一件工具，参数里是一条路径。
fn one(tool: &str, path: &str) -> Play {
    Play::calls(&[(tool, &serde_json::json!({ "path": path }).to_string())])
}

/// 场所 `venue` 里、工作区 `work/` 的会话：工作区这一级，没人能确认，属主是不是管理员照 `owner_is_admin`。
async fn session(
    home: &Home,
    script: &Script,
    kit: &Kit,
    venue: &str,
    owner_is_admin: bool,
) -> Handle {
    std::fs::create_dir_all(home.scratch.0.join("work")).expect("建得了工作区");
    std::fs::create_dir_all(home.scratch.0.join("elsewhere")).expect("建得了别处");
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: at(home, "work"),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        owner_is_admin,
        ..Lines::default()
    };
    home.create_full(script, &kit.catalog(), opening, lines)
        .await
}

/// 说一句，等这一轮说完；交回日志。
async fn turn(home: &Home, handle: &Handle) -> Vec<Event> {
    let mut pushes = watch(handle).await;
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    home.log(handle.id())
}

/// 日志里的工具结果，照先后。
fn results(log: &[Event]) -> Vec<ToolResult> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.clone()),
            _ => None,
        })
        .collect()
}

/// 一条工具结果的字。
fn text(result: &ToolResult) -> String {
    match result.blocks.as_slice() {
        [miyu_kernel::block::Block::Text(text)] => text.text.clone(),
        other => panic!("{other:?}"),
    }
}

fn shipped(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/core/permissions")
            .join(name),
    )
    .expect("出厂有")
}

#[tokio::test]
async fn a_group_session_only_touches_its_workspace_and_runs_no_command() {
    let home = Home::new();
    let kit = Kit::new();
    let inside = at(&home, "work/notes.md");
    let outside = at(&home, "elsewhere/secret.txt");
    let script = Script::new([
        one("read", &inside),
        one("write", &inside),
        one("read", &outside),
        one("write", &outside),
        Play::calls(&[("shell", "{\"command\": \"ls\"}")]),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, &kit, "qq:group:1", false).await;
    let log = turn(&home, &handle).await;
    assert_eq!(kit.read.calls().len(), 1, "工作区里的读放行：{log:#?}");
    assert_eq!(kit.write.calls().len(), 1, "工作区里的写放行");
    assert!(kit.shell.calls().is_empty(), "命令一律拒");
    let statuses: Vec<ToolStatus> = results(&log)
        .iter()
        .map(|result| result.status.clone())
        .collect();
    assert_eq!(
        statuses,
        [
            ToolStatus::Ok,
            ToolStatus::Ok,
            ToolStatus::Denied,
            ToolStatus::Denied,
            ToolStatus::Denied
        ]
    );
    let all = results(&log);
    assert_eq!(
        text(&all[2]),
        miyu_kernel::template::Template::parse(&shipped("outside-workspace.txt"))
            .expect("出厂的写法对")
            .render(&std::collections::BTreeMap::from([(
                "path",
                outside.as_str()
            )]))
            .expect("换得进"),
        "照出厂的那一句"
    );
    assert_eq!(
        all[2].human.as_ref().map(|said| said.key.as_str()),
        Some("core/permissions/outside-workspace")
    );
    assert_eq!(text(&all[4]), shipped("no-commands.txt"));
}

#[tokio::test]
async fn the_admins_own_chat_and_local_sessions_are_not_confined() {
    for (venue, owner_is_admin) in [("qq:private:10001", true), ("local", false)] {
        let home = Home::new();
        let kit = Kit::new();
        let outside = at(&home, "elsewhere/notes.md");
        let script = Script::new([one("read", &outside), Play::Says("好。")]);
        let handle = session(&home, &script, &kit, venue, owner_is_admin).await;
        turn(&home, &handle).await;
        assert_eq!(kit.read.calls().len(), 1, "{venue}：边界以外的读照旧放行");
    }
}
