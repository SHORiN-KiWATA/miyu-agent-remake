//! 叫停：丢掉这次调用的 future，阻塞线程里走目录的活就停下。

use std::sync::mpsc;
use std::time::{Duration, Instant};

use miyu_tool::Stop;

use super::{Fence, files};
use crate::blocking::blocking;

/// 一个用完就删的临时目录，里面有几个文件。
struct Temp(std::path::PathBuf);

impl Drop for Temp {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 什么都不挡的篱笆。
fn open() -> Fence {
    Fence {
        data_root: None,
        workspace: None,
    }
}

#[tokio::test]
async fn dropping_the_call_stops_the_walk() {
    let dir = Temp(std::env::temp_dir().join(format!("miyu-walk-{}", std::process::id())));
    for name in ["a.txt", "sub/b.txt"] {
        let path = dir.0.join(name);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, "x").expect("写得进");
    }
    let everything = files(&dir.0, open(), &Stop::default(), |_| true);
    assert_eq!(everything.len(), 2, "没叫停的走得完");
    let (started, wait) = mpsc::channel();
    let (done, result) = mpsc::channel();
    let root = dir.0.clone();
    let task = tokio::spawn(blocking(Stop::default(), move |stop| {
        started.send(()).expect("还在等");
        let begin = Instant::now();
        while !stop.stopped() && begin.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
        }
        let found = files(&root, open(), stop, |_| true);
        done.send((stop.stopped(), found.len())).expect("还在等");
    }));
    // 等的时候不占着跑异步任务的那一个线程：不然活根本派不出去。
    tokio::task::spawn_blocking(move || wait.recv_timeout(Duration::from_secs(10)))
        .await
        .expect("等得了")
        .expect("活开始了");
    task.abort();
    let (stopped, count) =
        tokio::task::spawn_blocking(move || result.recv_timeout(Duration::from_secs(20)))
            .await
            .expect("等得了")
            .expect("活停下了");
    assert!(stopped, "丢掉 future，旗就举起来");
    assert_eq!(count, 0, "举了旗，一步都不往下走");
}
