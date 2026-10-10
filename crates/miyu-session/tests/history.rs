//! 真的 `history`（施工 6-4）：会话把自己日志的只读入口、时区交给这次调用。压缩以后她调它，找得到压缩以前说的话，
//! 时刻照会话的时区写（开会话时东九区）。会话中途换时区那条路（`Handle::environment`）没人走，2026-10-08 项目主人定删了。

use std::path::Path;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Level, Permission, Said, ToolStatus};
use miyu_kernel::time::UtcOffset;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

fn base_system() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

#[tokio::test]
async fn after_a_compaction_she_finds_what_was_said_before_it() {
    // 测的是到线当场压：关掉提前压（施工 6-11 补起开着的会接着说，压缩挪到下一步，见内核的 `scenario/prepare_go.rs`）。
    let mut home = Home::new();
    home.configs = crate::support::routing::configs("[compaction]\nprepare = false\n", &[]);
    // 窗口 21242，压缩线 180（减掉预留 20000、窗口的 5% 1062）。带着基础系统八件工具，每次请求估出来都上千，比剧本报的 110 大，照估的算：第二轮一开头
    // 压一次，找回来以后又压一次（每一步至多压一次）。
    let script = Script::new([
        Play::Says("记下了：项目代号 BLUE-WHALE-7。"),
        Play::Says("<summary>用户交代了项目代号。</summary>"),
        Play::calls(&[("history", r#"{"query":"blue-whale"}"#)]),
        Play::Says("<summary>用户问代号，她翻记录找到了。</summary>"),
        Play::Says("代号是 BLUE-WHALE-7。"),
    ])
    .window(21_242);
    let cwd = home.scratch.0.join("work").to_string_lossy().into_owned();
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: cwd.clone(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &base_system(), opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("记一下项目代号"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    ask(&handle, "cmd-2", say("项目代号是什么？"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let log = home.log(handle.id());
    let compacted: Vec<u64> = log
        .iter()
        .filter(|event| matches!(event.body, Body::ContextCompacted(_)))
        .map(|event| event.seq.get())
        .collect();
    assert_eq!(compacted.len(), 2, "压了两次：{log:#?}");
    let said = log
        .iter()
        .find(|event| {
            matches!(&event.body, Body::MessageAssistant(reply)
                if reply.blocks.iter().any(|block| matches!(block, Block::Text(text) if text.text.contains("记下了"))))
        })
        .expect("第一轮她说了代号");
    assert!(said.seq.get() < compacted[0], "说代号那一条在压缩以前");
    let result = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.clone()),
            _ => None,
        })
        .expect("history 有结果");
    assert_eq!(result.status, ToolStatus::Ok);
    let text: String = result
        .blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只有字：{other:?}"),
        })
        .collect();
    // 照造会话时的时区写（测试的环境是东九区）。
    let when = said
        .at
        .local_minute(UtcOffset::from_minutes(540).expect("东九区在范围里"));
    assert_eq!(
        text,
        format!(
            "#{} {when} assistant: 记下了：项目代号 BLUE-WHALE-7。\n",
            said.seq.get()
        )
    );
    assert_eq!(
        result.human,
        Some(Said::new("software/basesystem/history/found/one").with("count", "1"))
    );
}
