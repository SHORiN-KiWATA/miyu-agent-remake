//! 追加并同步（`23-性能预算.md` 第二节「追加一步的事件并同步」，`07-存储.md` S4）：在进程里量，不经核心。
//!
//! 事件照大会话日志里真的那些，从第一条起一条一条追加进一份新日志，每条写一次、同步一次，和会话 actor 落盘走
//! 同一个函数（`miyu_store::log::SessionLog::append`）。一步落几条照这一步有几条，一条一次是最常见的、也是最贵的。

use std::path::Path;
use std::time::Instant;

use miyu_store::log::{SEGMENT_LIMIT, SessionLog, read_events};

use crate::stats::ms;

/// 读 `from` 的事件，前 `count` 条一条一条追加进 `scratch` 下的一份新日志，交回每一条的毫秒数。
///
/// # Errors
///
/// 读不了、建不了、写不进。
pub fn append(from: &Path, scratch: &Path, count: usize) -> Result<Vec<f64>, String> {
    let events = read_events(from).map_err(|e| format!("大会话的日志读不了：{e}"))?;
    if scratch.exists() {
        std::fs::remove_dir_all(scratch)
            .map_err(|e| format!("删不掉旧的 {}：{e}", scratch.display()))?;
    }
    let mut log = SessionLog::create(scratch, SEGMENT_LIMIT)
        .map_err(|e| format!("建不了 {}：{e}", scratch.display()))?;
    let mut took = Vec::with_capacity(count);
    for event in events.iter().take(count) {
        let began = Instant::now();
        log.append(std::slice::from_ref(event))
            .map_err(|e| format!("写不进：{e}"))?;
        took.push(ms(began.elapsed()));
    }
    Ok(took)
}
