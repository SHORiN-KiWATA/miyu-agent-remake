//! 真的 `read`（施工 4-4 上）：会话里她调它，工作区里的读得到、带行号，下一次请求里有；越界的读，在没人能确认的
//! 会话里被拒。

mod support;

use std::path::Path;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Level, Permission, ToolResult, ToolStatus};
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
    std::fs::write(home.scratch.0.join("other/b.txt"), "secret\n").expect("写得进");
    let outside = home
        .scratch
        .0
        .join("other/b.txt")
        .to_string_lossy()
        .into_owned();
    let script = Script::new([
        Play::calls(&[
            ("read", r#"{"path":"a.txt"}"#),
            ("read", &serde_json::json!({ "path": outside }).to_string()),
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
    };
    let handle = home.create_as(&script, &base_system(), opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("读一下"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let results = results(&home, &handle);
    assert_eq!(results.len(), 2);
    let (inside, outside) = if text(&results[0].blocks).contains("hello") {
        (&results[0], &results[1])
    } else {
        (&results[1], &results[0])
    };
    assert_eq!(inside.status, ToolStatus::Ok);
    assert_eq!(text(&inside.blocks), "     1\thello\n");
    assert_eq!(outside.status, ToolStatus::Denied, "越界要问人，没人能确认");
    assert!(!text(&outside.blocks).contains("secret"));
    // 她下一次请求里听到了。
    let requests = script.requests();
    let heard = requests[1].1.messages.iter().any(|message| {
        matches!(message, Message::Tool { blocks, .. } if text(blocks) == "     1\thello\n")
    });
    assert!(heard);
    // tools 数组里有 read，照资源里的说明。
    assert_eq!(requests[0].1.tools.len(), 1);
    assert_eq!(requests[0].1.tools[0].name, "read");
}
