//! 策略快照存进去、读回来（`docs/construction/3-6-策略快照（上）.md` 验收第 2 条）：从源码树的资源
//! 目录拼出软件工程师的快照，存成 blob，照哈希取回来重建；两份策略跑同一个剧本，每一次请求逐字节一样。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::facts::Environment;
use miyu_kernel::testkit::{Line, Stage};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_policy::{Snapshot, compose};
use miyu_store::blob::Blobs;
use miyu_store::resources::ResourceRoot;

/// 一个用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(
            std::env::temp_dir().join(format!("miyu-store-snapshot-{}-{n}", std::process::id())),
        )
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 源码树里的资源目录。
fn resources() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 照 `snapshot` 造策略，跑一段两轮的对话，交回每一次请求的规范字节。
fn requests(snapshot: Snapshot) -> Vec<Vec<u8>> {
    let environment = Environment {
        offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
        cwd: "~/src/miyu".to_string(),
    };
    let start = Timestamp::parse("2026-09-27T07:00:00.000Z").expect("时刻合写法");
    let mut stage = Stage::new(
        move || snapshot.policy().expect("出厂的快照造得出策略"),
        environment,
        start,
    );
    stage.model([Line::says("你好。"), Line::says("再见。")]);
    stage.say("hi");
    stage.say("bye");
    stage
        .requests()
        .iter()
        .map(|(_, request)| request.canonical_bytes())
        .collect()
}

#[test]
fn a_stored_snapshot_reads_back_and_gives_the_same_requests() {
    let scratch = Scratch::new();
    let snapshot = compose(
        "engineer",
        resources()
            .sources("engineer")
            .expect("出厂的软件工程师读得出来"),
        true,
    );
    let blobs = Blobs::new(scratch.0.join("blobs"));
    let hash = blobs.put(&snapshot.to_bytes()).expect("存得进去");
    assert_eq!(hash, snapshot.hash(), "存成 blob 的哈希就是快照的哈希");
    let loaded = Snapshot::from_bytes(&blobs.get(&hash).expect("取得回来")).expect("读得回来");
    assert_eq!(loaded, snapshot);

    let (before, after) = (requests(snapshot), requests(loaded));
    assert_eq!(before.len(), 2);
    assert_eq!(before, after, "重建的策略发出去的请求一字不差");
    let first = String::from_utf8(before[0].clone()).expect("请求是 UTF-8");
    assert!(
        first.contains(r#""system":"You are a helpful software engineer.""#),
        "{first}"
    );
}
