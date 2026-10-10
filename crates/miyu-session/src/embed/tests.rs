//! 关掉和排队（施工 R-5 四补）：排在拉起着的那一条后面的，轮到时已经关掉了，就交回用不了，不把换下来的又拉起；关掉了的不再
//! 核对包里的文件（卸包时核心要删它们，Windows 上开着的删不掉）。照真小程序的换在 `tests/embed_replace.rs`。

use std::path::PathBuf;
use std::time::Duration;

use super::*;

/// 小模型的模型清单能读、程序是一个不存在的：真去拉起的会交回「这一条算不出」，看得出拉没拉。文件照 `files` 算备到哪了。
fn embedder(files: Files) -> Embedder {
    let tiny = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../miyu-embed/tests/fixtures/tiny");
    let embedder = Embedder::new(EmbedSetup {
        program: Some(tiny.join("no-such-miyu-embed")),
        manifest: tiny.join("manifest.toml"),
        dir: tiny,
        idle: IDLE,
    });
    *embedder
        .shared
        .files
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = files;
    embedder
}

#[tokio::test]
async fn one_queued_while_it_is_shut_does_not_start_it_again() {
    let embedder = embedder(Files::Ready);
    let held = embedder.shared.slot.lock().await;
    let waiting = tokio::spawn({
        let embedder = embedder.clone();
        async move { embedder.embed("猫").await }
    });
    // 让它过了开头那一道、排到锁上。
    tokio::time::sleep(Duration::from_millis(50)).await;
    embedder.shared.shut.store(true, Ordering::SeqCst);
    drop(held);
    let got = waiting.await.expect("没崩");
    assert_eq!(
        got,
        Err(Unavailable::Off(
            "the embedding model was replaced".to_string()
        ))
    );
    assert!(!embedder.running().await);
}

#[tokio::test]
async fn a_shut_one_refuses_at_once_and_does_not_check_the_files() {
    let embedder = embedder(Files::Unchecked);
    embedder.shut().await;
    assert!(matches!(
        embedder.embed("猫").await,
        Err(Unavailable::Off(_))
    ));
    let files = embedder
        .shared
        .files
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    assert!(matches!(*files, Files::Unchecked), "{files:?}");
}
