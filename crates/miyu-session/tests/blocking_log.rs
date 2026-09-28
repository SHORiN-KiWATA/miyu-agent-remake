//! 阻塞线程里发的行带会话编号（施工 4-9 再补四上，`log.md` 第 5 条）：工具跑完，在阻塞线程里把效果存成 blob，存不
//! 进去的记一行 `WARN`；这一行原来没有会话编号。
//!
//! 全局装一个写进内存的订阅者：阻塞线程不是测试的线程，只装在测试线程上的接不到它发的行。全局的一个进程只能装一次，
//! 所以这个文件单独一个测试程序，只有一个测试。

mod support;

use std::path::Path;

use miyu_kernel::event::{Level, Permission};
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use support::*;

fn base_system() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

#[tokio::test]
async fn a_line_from_the_thread_that_stores_effects_carries_the_session() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let home = Home::outside_temp();
    let script = Script::new([
        Play::calls(&[("write", r#"{"file_path":"b.txt","content":"b\n"}"#)]),
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
    // 放 blob 的目录换成一个文件：改后的内容存不进去（哈希照样算出来交回，这一轮照常走完）。
    let blobs = home.root.blobs(&alice_account());
    std::fs::rename(&blobs, blobs.with_extension("moved")).expect("挪得开");
    std::fs::write(&blobs, b"").expect("写得进");
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("写一个"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let lines = memory.lines();
    let line = lines
        .iter()
        .find(|line| line.contains("effect content not stored"))
        .unwrap_or_else(|| panic!("记了存不进去：{lines:?}"));
    let expected = format!("WARN  session  {} effect content not stored", handle.id());
    assert!(line.contains(&expected), "{line}");
}
