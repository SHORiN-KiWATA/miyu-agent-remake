//! 真的撤销能撤掉压缩（施工 6-9）：第一轮打个招呼；第二轮她读了一个小文件、一个大文件，读完就过线，在这一轮里压，小
//! 文件重读进检查点；第三轮再说一句。撤掉第二轮，执行器从磁盘读回更早的日志，上下文回到压缩前；恢复把压缩放回来，不请求
//! 模型，重读的原文照 blob 取回，接着的请求里照样有。撤掉以后停了再载入，内核照日志认出那次压缩不算了。

use std::path::Path;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Level, Permission};
use miyu_kernel::id::TurnId;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::{Command, Outcome};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

fn base_system() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 请求里的全部字，连成一段。
fn words(request: &Request) -> String {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks }
            | Message::Assistant { blocks }
            | Message::Tool { blocks, .. } => blocks.iter(),
        })
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

/// 摘要：第二轮里压的那一次写的。
const SUMMARY: &str = "<summary>她读了两个文件。</summary>";

/// 重读进检查点的小文件，在检查点里的样子。
const SMALL: &str = "\">\nalpha\n\n</file>\n";

/// 第一轮打招呼；第二轮读 `a.txt`（小）和 `big.txt`（约 4 万字节），读完就过线，这一轮里压；第三轮再说一句。剧本后面
/// 接着 `more`。交回场地、剧本、会话，和压缩所在的那一轮。窗口 43000：线 10000，读了大文件就过线。
async fn compacted(more: Vec<Play>) -> (Home, Script, Handle, TurnId) {
    // 测的是到线当场压：关掉提前压（施工 6-11 补起开着的会接着说，压缩挪到下一步，见内核的 `scenario/prepare_go.rs`）。
    let mut home = Home::new();
    home.configs = crate::support::routing::configs("[compaction]\nprepare = false\n", &[]);
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    std::fs::write(work.join("a.txt"), "alpha\n").expect("写得进");
    let big: String = (0..1000)
        .map(|n| format!("{n:05} lorem ipsum dolor sit amet\n"))
        .collect();
    std::fs::write(work.join("big.txt"), &big).expect("写得进");
    let mut plays = vec![
        Play::Says("你好。"),
        Play::calls(&[
            ("read", r#"{"file_path":"a.txt"}"#),
            ("read", r#"{"file_path":"big.txt"}"#),
        ]),
        Play::Says(SUMMARY),
        Play::Says("好。"),
        Play::Says("嗯。"),
    ];
    plays.extend(more);
    let script = Script::new(plays).window(43_000);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &base_system(), opening).await;
    let mut pushes = watch(&handle).await;
    for (id, words) in [("cmd-1", "你好"), ("cmd-2", "读一下"), ("cmd-3", "接着来")] {
        ask(&handle, id, say(words)).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
    }
    let log = home.log(handle.id());
    let turns: Vec<TurnId> = log
        .iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| TurnId::new(event.seq))
        .collect();
    let compaction = log
        .iter()
        .find(|event| matches!(event.body, Body::ContextCompacted(_)))
        .expect("压了");
    assert_eq!(compaction.turn, Some(turns[1]), "压在第二轮里");
    (home, script, handle, turns[1])
}

/// 从 `turn` 起撤：接受了。
async fn undo_from(handle: &Handle, id: &str, turn: TurnId) {
    let undone = ask(handle, id, Command::Revert { turn: Some(turn) }).await;
    assert!(matches!(undone, Ok(Outcome::Accepted { .. })), "{undone:?}");
}

#[tokio::test]
async fn restoring_the_undone_compaction_brings_it_back_with_its_files() {
    // 场地要活到最后：会话目录、blob 都在它里面。
    let (_home, script, handle, turn) = compacted(vec![Play::Says("还在。")]).await;
    undo_from(&handle, "cmd-4", turn).await;
    let restored = ask(&handle, "cmd-5", Command::Unrevert).await;
    assert!(
        matches!(restored, Ok(Outcome::Accepted { .. })),
        "{restored:?}"
    );
    assert_eq!(script.requests().len(), 5, "撤销、恢复都不请求模型");
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-6", say("还在吗"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let requests = script.requests();
    let after = words(&requests.last().expect("有请求").1);
    assert!(after.contains("她读了两个文件"), "检查点回来了：{after}");
    assert!(after.contains(SMALL), "重读的原文取回来了：{after}");
    assert!(!after.contains("00999 lorem"), "压掉的大文件不在：{after}");
}

#[tokio::test]
async fn after_the_undo_and_a_reload_the_context_is_back_before_the_compaction() {
    let (home, script, handle, turn) = compacted(vec![Play::Says("好的。")]).await;
    undo_from(&handle, "cmd-4", turn).await;
    let id = handle.id().clone();
    stop(&handle).await;
    let handle = home.load_with(&id, &script, &base_system()).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-5", say("再说")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    // 回到压缩以前：第一轮还在，第二、三轮连同里面的压缩都撤掉了，没有检查点。
    let requests = script.requests();
    let after = words(&requests.last().expect("有请求").1);
    assert!(after.contains("你好"), "{after}");
    assert!(!after.contains("conversation-checkpoint"), "{after}");
    assert!(
        !after.contains("她读了两个文件"),
        "撤掉的压缩不算了：{after}"
    );
    for gone in ["读一下", "接着来", "00999 lorem"] {
        assert!(!after.contains(gone), "撤掉的不在：{after}");
    }
    assert_eq!(requests.len(), 6, "没有再压：一轮一次请求");
}
