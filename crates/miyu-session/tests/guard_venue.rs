//! 在场所里做的事（访问类别 `venue`，施工 O-31 前，`session/guard.md` 第四条）：场所会话里放行、不问人，只读开着、没人能确认
//! 也一样；本机的会话拒绝，`by` 是 `permissions`，写给她的是出厂的那一句，说法 `core/permissions/not-in-venue`，工具没跑。载入以后
//! 照样（照 `session.created` 的场所认）。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::{Body, Level, Permission, ToolStatus};
use miyu_kernel::id::VenueId;
use miyu_kernel::origin::By;
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 在场所里做的一件假工具。
fn kick() -> Arc<Fake> {
    Fake::new("kick", Access::Venue, Act::Echo)
}

/// 场所 `venue` 里的会话：只读开着、没人能确认，工具目录只有 `tool`。
async fn session(home: &Home, script: &Script, tool: &Arc<Fake>, venue: &str) -> Handle {
    let catalog = Catalog::new([Arc::clone(tool) as Arc<dyn Tool>]).expect("合写法");
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: true,
        },
        attended: false,
        ..Opening::default()
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        ..Lines::default()
    };
    home.create_full(script, &catalog, opening, lines).await
}

/// 说一句，等这一轮说完；交回日志。
async fn turn(home: &Home, handle: &Handle) -> Vec<miyu_kernel::event::Event> {
    let mut pushes = watch(handle).await;
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    home.log(handle.id())
}

#[tokio::test]
async fn a_venue_action_runs_in_a_venue_even_read_only_and_unattended() {
    for venue in ["qq:group:1", "qq:private:10001"] {
        let home = Home::new();
        let tool = kick();
        let script = Script::new([Play::calls(&[("kick", "{}")]), Play::Says("好。")]);
        let handle = session(&home, &script, &tool, venue).await;
        let log = turn(&home, &handle).await;
        assert_eq!(tool.calls().len(), 1, "{venue}：放行，不问人：{log:#?}");
        assert!(
            !log.iter()
                .any(|event| matches!(event.body, Body::ApprovalRequested(_))),
            "{venue}：不问人"
        );
    }
}

#[tokio::test]
async fn a_venue_action_is_refused_outside_a_venue() {
    let home = Home::new();
    let tool = kick();
    let script = Script::new([Play::calls(&[("kick", "{}")]), Play::Says("好。")]);
    let handle = session(&home, &script, &tool, "local").await;
    let log = turn(&home, &handle).await;
    assert!(tool.calls().is_empty(), "没跑");
    let (result, by) = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some((result.clone(), event.by.clone())),
            _ => None,
        })
        .expect("有一条工具结果");
    assert!(
        matches!(&by, By::Module(module) if module.id.as_str() == "permissions"),
        "{by:?}"
    );
    assert_eq!(result.status, ToolStatus::Denied);
    let shipped = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources/core/permissions/not-in-venue.txt"),
    )
    .expect("读得出");
    let text: String = result
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect();
    assert_eq!(text, shipped);
    assert_eq!(
        result.human.as_ref().map(|said| said.key.as_str()),
        Some("core/permissions/not-in-venue")
    );
}

#[tokio::test]
async fn a_loaded_session_still_knows_where_it_is() {
    for (venue, runs) in [("qq:group:1", true), ("local", false)] {
        let home = Home::new();
        let tool = kick();
        let script = Script::new([
            Play::Says("在。"),
            Play::calls(&[("kick", "{}")]),
            Play::Says("好。"),
        ]);
        let handle = session(&home, &script, &tool, venue).await;
        turn(&home, &handle).await;
        stop(&handle).await;
        let catalog = Catalog::new([Arc::clone(&tool) as Arc<dyn Tool>]).expect("合写法");
        let loaded = home.load_with(handle.id(), &script, &catalog).await;
        let mut pushes = watch(&loaded).await;
        ask(&loaded, "cmd-2", say("再来")).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
        assert_eq!(
            tool.calls().len(),
            usize::from(runs),
            "{venue}：载入以后照样"
        );
    }
}
