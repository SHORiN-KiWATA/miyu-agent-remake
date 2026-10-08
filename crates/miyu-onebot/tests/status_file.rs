//! 状态文件（施工 O-18，`onebot.md` 第一条「状态文件」）：桥听上了就写进程号、两个实际的端口、NapCat 没连着；NapCat 连上以后
//! 说号、问到了再说实现和版本，断了说没连着。`/apply` 换端口跟着换由 `apply.rs` 守着。核心是替身。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_onebot::status_file;
use miyu_store::root::DataRoot;

use crate::support::fake_core::fake_core;
use crate::support::*;

/// 等到状态文件读得懂、合 `wanted`：最多 10 秒。
pub async fn until_file(root: &DataRoot, wanted: impl Fn(&Value) -> bool) -> Value {
    within("状态文件", async {
        loop {
            if let Some(read) = status_file::read(root)
                && wanted(&read)
            {
                return read;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
}

#[tokio::test]
async fn the_status_file_follows_napcat() {
    let (dir, root) = temp_root();
    let _core = fake_core(&root);
    assert_eq!(
        status_file::path(&root),
        root.state()
            .join("packages")
            .join("onebot")
            .join("status.json"),
        "在核心给的工作目录里"
    );
    let bridge = start(serve(root.clone(), settings())).await;
    let pid = std::process::id();
    let first = until_file(&root, |file| file["pid"] == pid).await;
    assert_eq!(
        first,
        json!({"pid": pid, "listen": bridge.port, "web": bridge.web, "napcat": {"connected": false}})
    );
    let mut napcat = admitted(bridge.port, "/ws", Auth::Bearer(TOKEN), Some(BOT)).await;
    napcat.version().await;
    let connected = until_file(&root, |file| file["napcat"]["implementation"].is_string()).await;
    assert_eq!(
        connected["napcat"],
        json!({"connected": true, "self_id": BOT.to_string(), "implementation": "NapCat.Onebot", "version": "4.8.0"})
    );
    napcat.close().await;
    until_file(&root, |file| file["napcat"] == json!({"connected": false})).await;
    // 一行一份 JSON，末尾带换行。
    let raw = std::fs::read_to_string(status_file::path(&root)).expect("读得了");
    assert!(raw.ends_with("}\n") && raw.lines().count() == 1, "{raw}");
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn a_bot_known_only_from_its_first_event_is_written_too() {
    let (dir, root) = temp_root();
    let _core = fake_core(&root);
    let bridge = start(serve(root.clone(), settings())).await;
    let mut napcat = admitted(bridge.port, "/ws", Auth::Bearer(TOKEN), None).await;
    let pid = std::process::id();
    let unknown = until_file(&root, |file| file["pid"] == pid).await;
    assert_eq!(
        unknown["napcat"],
        json!({"connected": false}),
        "号还没认出来"
    );
    napcat
        .send(json!({"time": 1, "self_id": BOT, "post_type": "meta_event", "meta_event_type": "heartbeat"}))
        .await;
    let known = until_file(&root, |file| file["napcat"]["connected"] == true).await;
    assert_eq!(known["napcat"]["self_id"], BOT.to_string());
    napcat.close().await;
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}
