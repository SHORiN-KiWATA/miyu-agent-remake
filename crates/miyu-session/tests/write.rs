//! 真的 `write`（施工 4-6 上）、`edit`（施工 4-6 中）：会话里她先读后写，日志里两次结果的效果对得上，blob 里存着改前
//! 改后的内容；新建的不用先读；没读过就写的被拒，读过以后可以写，会话重新载入以后她读过的照样算数；改完一次接着改，
//! 不用重读。

mod support;

use std::path::Path;

use miyu_kernel::event::{
    Body, Effect, FileChanged, Level, Permission, Said, ToolResult, ToolStatus,
};
use miyu_kernel::id::ContentHash;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

use support::*;

fn base_system() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
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

/// 在场地的 `work/` 里干活，工作区这一级，没人能确认。
fn opening(home: &Home) -> Opening {
    Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: work(home),
    }
}

/// 场地里的工作区。
fn work(home: &Home) -> String {
    home.scratch.0.join("work").to_string_lossy().into_owned()
}

/// 说一句，等这一轮结束。
async fn talk(handle: &miyu_session::Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

#[tokio::test]
async fn she_reads_then_writes_and_the_log_keeps_both_contents() {
    let home = Home::outside_temp();
    let file = home.scratch.0.join("work/a.txt");
    std::fs::write(&file, "old\n").expect("写得进");
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("write", r#"{"file_path":"a.txt","content":"new\n"}"#)]),
        Play::calls(&[("write", r#"{"file_path":"b.txt","content":"b\n"}"#)]),
        Play::Says("好。"),
    ]);
    let handle = home
        .create_as(&script, &base_system(), opening(&home))
        .await;
    talk(&handle, "cmd-1", "改一下").await;
    let results = results(&home, &handle);
    assert_eq!(results.len(), 3);
    assert!(
        results.iter().all(|result| result.status == ToolStatus::Ok),
        "{results:?}"
    );
    assert_eq!(std::fs::read(&file).expect("读得出"), b"new\n");
    let real = std::fs::canonicalize(&file)
        .expect("在")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        results[1].effects,
        [Effect::FileChanged(FileChanged {
            path: real,
            before: Some(ContentHash::of(b"old\n")),
            after: ContentHash::of(b"new\n"),
        })]
    );
    let blobs = Blobs::new(home.root.blobs(&alice_account()));
    assert_eq!(
        blobs.get(&ContentHash::of(b"old\n")).expect("存了改前的"),
        b"old\n"
    );
    assert_eq!(
        blobs.get(&ContentHash::of(b"new\n")).expect("存了改后的"),
        b"new\n"
    );
    // 新建的不用先读，改前是空的。
    match &results[2].effects[..] {
        [Effect::FileChanged(changed)] => {
            assert_eq!(changed.before, None);
            assert_eq!(changed.after, ContentHash::of(b"b\n"));
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn what_she_read_still_counts_after_the_session_is_loaded_again() {
    let home = Home::outside_temp();
    let file = home.scratch.0.join("work/a.txt");
    std::fs::write(&file, "old\n").expect("写得进");
    let write = r#"{"file_path":"a.txt","content":"new\n"}"#;
    let script = Script::new([
        Play::calls(&[("write", write)]),
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::Says("读过了。"),
    ]);
    let handle = home
        .create_as(&script, &base_system(), opening(&home))
        .await;
    talk(&handle, "cmd-1", "改一下").await;
    let first = results(&home, &handle);
    assert_eq!(first[0].status, ToolStatus::Error, "没读过就写，不让");
    assert_eq!(
        first[0].human,
        Some(Said::new("software/basesystem/common/not-read"))
    );
    assert_eq!(std::fs::read(&file).expect("读得出"), b"old\n", "没写");
    // 停下再载入：她读过的从日志里重建，不用再读一遍就能写。
    let session = handle.id().clone();
    stop(&handle).await;
    let script = Script::new([Play::calls(&[("write", write)]), Play::Says("改好了。")]);
    let handle = home
        .load_at(&session, &script, &base_system(), &work(&home))
        .await;
    talk(&handle, "cmd-2", "现在改").await;
    let all = results(&home, &handle);
    let last = all.last().expect("有结果");
    assert_eq!(last.status, ToolStatus::Ok, "{all:?}");
    assert_eq!(std::fs::read(&file).expect("读得出"), b"new\n");
}

#[tokio::test]
async fn she_reads_then_edits_twice_without_reading_again() {
    let home = Home::outside_temp();
    let file = home.scratch.0.join("work/a.txt");
    std::fs::write(&file, "one\ntwo\n").expect("写得进");
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[(
            "edit",
            r#"{"file_path":"a.txt","edits":[{"old_string":"two","new_string":"2"}]}"#,
        )]),
        Play::calls(&[(
            "edit",
            r#"{"file_path":"a.txt","old_string":"one","new_string":"1"}"#,
        )]),
        Play::Says("好。"),
    ]);
    let handle = home
        .create_as(&script, &base_system(), opening(&home))
        .await;
    talk(&handle, "cmd-1", "改一下").await;
    let results = results(&home, &handle);
    assert!(
        results.iter().all(|result| result.status == ToolStatus::Ok),
        "{results:?}"
    );
    assert_eq!(std::fs::read(&file).expect("读得出"), b"1\n2\n");
    let real = std::fs::canonicalize(&file)
        .expect("在")
        .to_string_lossy()
        .into_owned();
    let changed = |before: &[u8], after: &[u8]| {
        Effect::FileChanged(FileChanged {
            path: real.clone(),
            before: Some(ContentHash::of(before)),
            after: ContentHash::of(after),
        })
    };
    assert_eq!(results[1].effects, [changed(b"one\ntwo\n", b"one\n2\n")]);
    assert_eq!(
        results[2].effects,
        [changed(b"one\n2\n", b"1\n2\n")],
        "第一次改完，她看过的就是改后的：第二次不用重读"
    );
    let blobs = Blobs::new(home.root.blobs(&alice_account()));
    assert_eq!(
        blobs.get(&ContentHash::of(b"1\n2\n")).expect("存了改后的"),
        b"1\n2\n"
    );
}
