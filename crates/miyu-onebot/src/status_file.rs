//! 桥的状态文件（`onebot.md` 第一条「状态文件」，施工 O-18）：`<数据根>/state/packages/onebot/status.json`，一行 JSON：
//!
//! ```json
//! {"pid": 12345, "listen": 8301, "web": 8302, "napcat": {"connected": true, "self_id": "30003"}}
//! ```
//!
//! - 桥的进程号、实际听的两个端口（`/apply` 换过的照换过的）、NapCat 的样子（同 WebUI 的 `/status`，`Bots::napcat`）。
//! - 两个端口听上以后写一次；之后有人叫一声（NapCat 连上、断开、认出号、问到实现，`/apply` 换了端口）就照这一刻再写。和磁盘上
//!   一样的不写，先写临时文件再改名盖上（`miyu_store::generated::write`）：读的人看不到写了一半的。写不进记一行，桥照跑。
//! - 照数据根算位置，不照工作目录的相对路径：核心给的工作目录就是这个目录（`extensions.md`「怎么走」第 1 条），在进程里跑的
//!   测试也不会写进源码树（「施工时定的」第 23 条）。
//! - 桥退出不删：`miyu onebot status` 只在核心说桥在跑、进程号对得上时用它（`crate::control`）。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde_json::{Value, json};
use tokio::sync::Notify;

use miyu_store::root::DataRoot;

use crate::serve::Failure;
use crate::web::Web;
use crate::{PACKAGE, TARGET};

/// 文件名：在这个包放状态的目录里。
const FILE: &str = "status.json";

/// 数据根 `root` 里桥的状态文件在哪。
pub fn path(root: &DataRoot) -> PathBuf {
    root.state().join("packages").join(PACKAGE).join(FILE)
}

/// 读数据根 `root` 里的状态文件；没有的、读不了的、不是 JSON 的是空的。
pub fn read(root: &DataRoot) -> Option<Value> {
    let bytes = std::fs::read(path(root)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// 照 `web` 这一刻的样子写进 `path`，之后每叫一声 `changed` 再写一次。一直写下去，桥停下时随一组任务一起停；交回的类型照
/// 那一组（`crate::serve`）。
pub(crate) async fn keep(path: PathBuf, web: Arc<Web>, changed: Arc<Notify>) -> Option<Failure> {
    loop {
        let now = json!({
            "pid": std::process::id(),
            "listen": web.listen.load(Ordering::Relaxed),
            "web": web.port.load(Ordering::Relaxed),
            "napcat": web.bots.napcat(),
        });
        let bytes = format!("{now}\n").into_bytes();
        let target = path.clone();
        match tokio::task::spawn_blocking(move || miyu_store::generated::write(&target, &bytes))
            .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => {
                tracing::warn!(target: TARGET, file = %path.display(), error = %error, "status not written");
            }
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "status not written");
            }
        }
        changed.notified().await;
    }
}
