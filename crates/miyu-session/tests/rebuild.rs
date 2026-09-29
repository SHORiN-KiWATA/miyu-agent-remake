//! 真的压后重建（施工 6-5）：第一轮她读了一个小文件、一个大文件，第二轮一开头就压。执行器照内核交的重读从磁盘读，小的
//! 存成 blob、原样放进检查点，大的只进清单；压完的主请求里看得到小文件的原文。停了再载入，执行器照最近一个检查点交回
//! 原文，接着的请求里照样有。

mod support;

use std::path::Path;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, ContextCompacted, Level, Permission};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

use support::*;

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
    // 窗口 43000：线 10000，压完的整份请求至多 5000，放得下小文件。
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
    .window(43_000);
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
    // 压完的主请求里有小文件的原文。
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
