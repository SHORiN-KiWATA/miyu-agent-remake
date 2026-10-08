//! `resources/software/onebot/bridge.json`（`onebot.md`「施工时定的」第 15 条）：桥自己的数放在它的软件包里，不进配置
//! 清单（照网页软件的 `web.json`，`web-module.md`「起草时定的」第 25 条）。起来时读一次。
//!
//! 是数据，不发给模型，不进登记簿（`xtask/src/ledger.rs` 只豁免这一份文件：这个包以后要放给模型看的字）。

use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

/// 文件在资源目录里的位置。
pub const FILE: &str = "software/onebot/bridge.json";

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tuning {
    /// NapCat 连得进来的路径（第 2 条），别的 404。
    pub paths: Vec<String>,
    /// 一次 OneBot 调用等回应最多几秒（第 4 条）。
    pub call_timeout_seconds: u64,
    /// 往一条 NapCat 的连接写，最多攒几帧没写出去；满了，写的一方等着。至少 1。
    pub write_queue: usize,
    /// 读出来的私聊最多攒几条没交给跟核心的那一头；满了，读 NapCat 的那一头等着。至少 1。
    pub inbound_queue: usize,
    /// 接不了 TCP 连接（打开的文件太多这类）时，歇几毫秒再接，不空转（照核心的规矩）。
    pub accept_retry_millis: u64,
}

impl Tuning {
    /// 读资源目录 `resources` 里的 [`FILE`]。
    ///
    /// # Errors
    ///
    /// 读不了、不是这个形状、队列写了 0（建不了队列）：原话里说是哪个文件。
    pub fn load(resources: &Path) -> Result<Tuning, String> {
        let path = resources.join(FILE);
        let bad = |why: String| format!("{} not readable: {why}", path.display());
        let text = std::fs::read_to_string(&path).map_err(|error| bad(error.to_string()))?;
        let tuning: Tuning = serde_json::from_str(&text).map_err(|error| bad(error.to_string()))?;
        if tuning.write_queue == 0 || tuning.inbound_queue == 0 {
            return Err(bad(
                "write_queue and inbound_queue must be at least 1".to_string()
            ));
        }
        Ok(tuning)
    }

    /// 一次调用等回应最多多久。
    pub fn call_timeout(&self) -> Duration {
        Duration::from_secs(self.call_timeout_seconds)
    }

    /// 接不了连接时歇多久。
    pub fn accept_retry(&self) -> Duration {
        Duration::from_millis(self.accept_retry_millis)
    }
}
