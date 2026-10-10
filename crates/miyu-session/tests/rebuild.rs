//! 真的压后重建（施工 6-5）：第一轮她读了一个小文件、一个大文件，读完就过线，这一轮里接着压（摘要请求回的是剧本的第
//! 二句）。执行器照内核交的重读从磁盘读，小的存成 blob、原样放进检查点，大的只进清单；第二轮的请求里看得到小文件的原文。
//! 停了再载入，内核要回检查点里重读过的原文（施工 6-9），接着的请求里照样有。检查点里代码写的几段不列还在跑的任务（施工
//! 7-8 补）。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, ContextCompacted, Level, Permission};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Command;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Tool};

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

#[tokio::test]
async fn a_small_file_read_before_the_cut_comes_back_and_survives_a_reload() {
    let home = Home::new();
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    std::fs::write(work.join("a.txt"), "alpha\n").expect("写得进");
    // 一千行、约 4 万字节：读它就过线，一个也超过单个 5000 token，只进清单。
    let big: String = (0..1000)
        .map(|n| format!("{n:05} lorem ipsum dolor sit amet\n"))
        .collect();
    std::fs::write(work.join("big.txt"), &big).expect("写得进");
    // 窗口 32000（重读要窗口不小于 32000）：线 10400（减掉预留 20000、窗口的 5% 1600），压完的整份请求至多 5000，放得下小文件。
    let script = Script::new([
        Play::calls(&[
            ("read", r#"{"file_path":"a.txt"}"#),
            ("read", r#"{"file_path":"big.txt"}"#),
        ]),
        Play::Says("好。"),
        Play::Says("<summary>她读了两个文件。</summary>"),
        Play::Says("嗯。"),
        Play::Says("还在。"),
    ])
    .window(32_000);
    let cwd = work.to_string_lossy().into_owned();
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
    ask(&handle, "cmd-1", say("读一下"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    ask(&handle, "cmd-2", say("接着来"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let id = handle.id().clone();
    let compacted: ContextCompacted = home
        .log(&id)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .expect("压了");
    // 重读了小的，照清单的写法；原文存进了这个会话的 blob。
    assert_eq!(compacted.restored.len(), 1, "{compacted:?}");
    assert!(compacted.restored[0].path.ends_with("a.txt"));
    assert_eq!(compacted.restored[0].blob, ContentHash::of(b"alpha\n"));
    let blobs = Blobs::new(home.root.blobs(&alice_account()));
    assert_eq!(
        blobs
            .get(&ContentHash::of(b"alpha\n"))
            .expect("存进了 blob"),
        b"alpha\n"
    );
    // 大的进清单，清单里两个都有，还有取回指路。
    assert!(
        compacted.notes.contains("Not shown again"),
        "{}",
        compacted.notes
    );
    assert!(compacted.notes.contains("big.txt"), "{}", compacted.notes);
    assert!(
        compacted.notes.contains("history still finds them"),
        "{}",
        compacted.notes
    );
    // 压完以后第二轮的请求里有小文件的原文。
    let requests = script.requests();
    let after = &requests[3].1;
    assert!(
        words(after).contains("\">\nalpha\n\n</file>\n"),
        "{}",
        words(after)
    );
    // 停了再载入：接着的请求里照样有。
    stop(&handle).await;
    let handle = home.load_with(&id, &script, &base_system()).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-3", say("还在吗"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let requests = script.requests();
    let reloaded = &requests.last().expect("有请求").1;
    assert!(
        words(reloaded).contains("\">\nalpha\n\n</file>\n"),
        "{}",
        words(reloaded)
    );
}

/// 检查点里不列还在跑的任务（施工 7-8 补，`compaction.md` 第八条第 4 条）：第一轮说一大段、放一个后台命令，再手动压缩，
/// 命令还在跑。照出厂的资源，代码写的几段只有取回指路：她派过什么、还在跑什么，交给摘要记。
#[tokio::test]
async fn a_command_still_running_is_not_listed_in_the_checkpoint() {
    let home = Home::new();
    let held = Held::new(&[]);
    let start = Fake::new("start", Access::Read, Act::Background(Arc::clone(&held)));
    let tools = Catalog::new([start as Arc<dyn Tool>]).expect("合写法");
    // 窗口大到不会自动压。出厂的尾巴是 16000 token：第一句说七万个字，一组就超了尾巴，压得掉它。
    let script = Script::new([
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::Says("<summary>她放了一个后台命令，还在跑。</summary>"),
    ])
    .window(1_000_000);
    let handle = home.create_with(&script, &tools).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say(&"x".repeat(70_000)))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let compact = Command::Compact { instructions: None };
    ask(&handle, "cmd-2", compact).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert!(home.jobs.running(), "压的时候命令还在跑");
    let compacted: ContextCompacted = home
        .log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .expect("压了");
    let retrieve = include_str!("../../../resources/core/compaction/notes-retrieve.txt")
        .replace("{upto}", &compacted.upto.to_string());
    assert_eq!(compacted.notes, retrieve, "代码写的几段只有取回指路");
    stop(&handle).await;
}
