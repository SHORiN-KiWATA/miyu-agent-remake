//! 测试程序旁边链 `miyu-onebot`（`support/spawning.rs` 的 [`link_beside`]，「施工时定的」第 28 条）：几份测试程序同时去链
//! 同一个（并着跑同一个测试程序，O-23 下撞过 `File exists`），都当成功，链出来的是同一个文件。

use std::path::PathBuf;
use std::sync::{Arc, Barrier};

use crate::support::spawning::link_beside;

/// 同时去链的有几个。
const AT_ONCE: usize = 8;

/// 链几轮：每轮一个新的目标，几个一起从「还没有」开始抢。
const ROUNDS: usize = 20;

#[test]
fn several_linking_at_once_all_succeed() {
    let dir = std::env::temp_dir().join(format!("miyu-onebot-linking-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了目录");
    let source = dir.join("source");
    std::fs::write(&source, b"the program").expect("写得进");
    for round in 0..ROUNDS {
        let path = dir.join(format!("linked-{round}"));
        let gate = Arc::new(Barrier::new(AT_ONCE));
        let threads: Vec<_> = (0..AT_ONCE)
            .map(|_| {
                let (gate, source, path): (_, PathBuf, PathBuf) =
                    (Arc::clone(&gate), source.clone(), path.clone());
                std::thread::spawn(move || {
                    gate.wait();
                    link_beside(&source, &path);
                })
            })
            .collect();
        for thread in threads {
            assert!(thread.join().is_ok(), "第 {round} 轮有一个链不上");
        }
        assert_eq!(
            std::fs::read(&path).expect("链上了"),
            b"the program",
            "第 {round} 轮"
        );
    }
    std::fs::remove_dir_all(&dir).expect("删得掉");
}
