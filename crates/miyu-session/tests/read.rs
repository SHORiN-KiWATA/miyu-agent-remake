//! 真的 `read`（施工 4-4 上，4-4 下改了参数名和行号）：会话里她调它，工作区里的读得到、带行号，下一次请求里有；边界以外
//! 的也读得到，没人能确认也不用问（施工 5-4 上，原来被拒）；数据根里的被拒。

mod support;

use std::path::Path;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Effect, FileRead, Level, Permission, Said, ToolResult, ToolStatus};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use support::*;

fn base_system() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

fn text(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

fn results(home: &Home, handle: &miyu_session::Handle) -> Vec<ToolResult> {
    home.log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn she_reads_a_file_in_the_workspace_and_hears_it() {
    let home = Home::outside_temp();
    std::fs::write(home.scratch.0.join("work/a.txt"), "hello\n").expect("写得进");
    std::fs::write(home.scratch.0.join("other/b.txt"), "far\n").expect("写得进");
    std::fs::write(
        home.root.path().join("marker"),
        "marker-content-never-read\n",
    )
    .expect("写得进");
    let outside = home
        .scratch
        .0
        .join("other/b.txt")
        .to_string_lossy()
        .into_owned();
    let marker = home
        .root
        .path()
        .join("marker")
        .to_string_lossy()
        .into_owned();
    let script = Script::new([
        Play::calls(&[
            ("read", r#"{"file_path":"a.txt"}"#),
            (
                "read",
                &serde_json::json!({ "file_path": outside }).to_string(),
            ),
            (
                "read",
                &serde_json::json!({ "file_path": marker }).to_string(),
            ),
        ]),
        Play::Says("好。"),
    ]);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &base_system(), opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("读一下"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let results = results(&home, &handle);
    assert_eq!(results.len(), 3);
    // 一起派的只读调用各跑各的，结果的先后不一定：照内容找。
    let find = |said: &str| {
        results
            .iter()
            .find(|result| text(&result.blocks).contains(said))
            .unwrap_or_else(|| panic!("有一条含「{said}」：{results:?}"))
    };
    let (inside, outside, data) = (find("hello"), find("far"), find("Miyu's own data"));
    assert_eq!(inside.status, ToolStatus::Ok);
    assert_eq!(text(&inside.blocks), "1\thello\n");
    // 工具交的给人看的说法，经会话记进日志（施工 4-5 上）；数据根里被拒的，是权限策略写的那一句的说法。
    assert_eq!(
        inside.human,
        Some(Said::new("software/basesystem/read/lines/one").with("count", "1"))
    );
    assert_eq!(outside.status, ToolStatus::Ok, "边界以外的读不用问");
    assert_eq!(text(&outside.blocks), "1\tfar\n");
    assert_eq!(data.status, ToolStatus::Denied, "数据根哪一级都不能碰");
    assert_eq!(
        data.human,
        Some(Said::new("core/permissions/forbidden").with("path", marker.as_str()))
    );
    // 比内容，不比「secret」：工作树的路径里可能就有这个词（施工 8-5 的 `.worktrees/8-5-secrets` 撞上过）。
    assert!(!text(&data.blocks).contains("marker-content-never-read"));
    // 读到的报 `file.read`（施工 4-6 上）：真实的位置、读了哪几行、整份的哈希。没读的没有效果。
    let real = std::fs::canonicalize(home.scratch.0.join("work/a.txt")).expect("在");
    assert_eq!(
        inside.effects,
        [Effect::FileRead(FileRead {
            path: real.to_string_lossy().into_owned(),
            lines: Some([1, 1]),
            hash: ContentHash::of(b"hello\n"),
        })]
    );
    assert!(data.effects.is_empty());
    // 她下一次请求里听到了。
    let requests = script.requests();
    let heard = requests[1].1.messages.iter().any(
        |message| matches!(message, Message::Tool { blocks, .. } if text(blocks) == "1\thello\n"),
    );
    assert!(heard);
    // tools 数组里是基础系统的十二件（本机的主会话有 `sessions`，施工 C-3），照名字排，照资源里的说明。
    let names: Vec<&str> = requests[0]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "edit",
            "glob",
            "grep",
            "history",
            "jobs",
            "read",
            "send_message",
            "sessions",
            "shell",
            "subagent",
            "trash",
            "write"
        ]
    );
}
