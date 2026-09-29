//! 真的 `glob`、`grep`（施工 4-4 下）：会话里她调它们，工作区里的找得到、搜得到，结果落盘；边界以外的也搜得到，
//! 没人能确认也不用问（施工 5-4 上，原来被拒）；从上面往下搜，不进数据根。

mod support;

use std::path::Path;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Level, Permission, ToolResult, ToolStatus};
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

#[tokio::test]
async fn she_finds_and_searches_in_the_workspace_and_outside() {
    let home = Home::outside_temp();
    std::fs::write(home.scratch.0.join("work/a.txt"), "hello\n").expect("写得进");
    std::fs::write(home.scratch.0.join("other/b.txt"), "hello far\n").expect("写得进");
    let outside = home.scratch.0.join("other").to_string_lossy().into_owned();
    let script = Script::new([
        Play::calls(&[
            ("glob", r#"{"pattern":"*.txt"}"#),
            ("grep", r#"{"pattern":"hel+o","output_mode":"content"}"#),
            (
                "grep",
                &serde_json::json!({ "pattern": "far", "path": outside }).to_string(),
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
    ask(&handle, "cmd-1", say("找一下"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    // 落了盘的结果：三次调用照先后各一条。
    let results: Vec<ToolResult> = home
        .log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 3);
    let texts: Vec<String> = results.iter().map(|result| text(&result.blocks)).collect();
    let found = texts.iter().position(|text| text == "a.txt\n");
    let searched = texts.iter().position(|text| text == "a.txt:1:hello\n");
    let (Some(found), Some(searched)) = (found, searched) else {
        panic!("工作区里的找得到、搜得到：{texts:?}");
    };
    assert_eq!(results[found].status, ToolStatus::Ok);
    assert_eq!(results[searched].status, ToolStatus::Ok);
    let outside = (0..3)
        .find(|at| *at != found && *at != searched)
        .expect("第三条");
    assert_eq!(
        results[outside].status,
        ToolStatus::Ok,
        "边界以外的搜不用问：{texts:?}"
    );
    assert!(texts[outside].contains("b.txt"), "{texts:?}");
}

#[tokio::test]
async fn searching_from_above_never_goes_into_miyus_own_data() {
    let home = Home::outside_temp();
    // 数据根就在场地的 data/ 下：会话日志里也有她这次调用写下的这个词。
    std::fs::write(home.scratch.0.join("data/needle.txt"), "zebra-7731\n").expect("写得进");
    std::fs::write(home.scratch.0.join("other/z.txt"), "zebra-7731\n").expect("写得进");
    let top = home.scratch.0.to_string_lossy().into_owned();
    let script = Script::new([
        Play::calls(&[(
            "grep",
            &serde_json::json!({ "pattern": "zebra-7731", "path": top }).to_string(),
        )]),
        Play::Says("好。"),
    ]);
    // 完全放开的级别：搜外面不用问，挡在数据根外面的只有工具自己。
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
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
    ask(&handle, "cmd-1", say("搜一下"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let results: Vec<ToolResult> = home
        .log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, ToolStatus::Ok);
    let real = std::fs::canonicalize(home.scratch.0.join("other/z.txt")).expect("在");
    let real = real.to_string_lossy();
    let shown = real.strip_prefix(r"\\?\").unwrap_or(&real);
    assert_eq!(text(&results[0].blocks), format!("{shown}\n"));
}
