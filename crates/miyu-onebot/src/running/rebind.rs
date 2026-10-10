//! 推来的端口变化（`onebot.md` 第一条「怎么走」第 1 条、「施工时定的」第 45、167 条；施工 O-20，O-28 下从 `web/apply.rs`
//! 挪过来）：照桥手里最新的配置换 NapCat 的端口，不读盘。
//!
//! 1. 配置里写的（`crate::current`）和上一次照的（[`Running::applied`]）比，变了的开新的（[`crate::serve::bind`]），开上了才
//!    交给 `serve` 换掉旧的，状态文件跟着写。开不了的不换，旧的照旧开着，桥不会落到哪个端口都不听；也不另试：人换一个再存，
//!    配置变了核心再推（第 167 条）。
//! 2. 已经接进来的连接（NapCat 的那一条）不断：换掉的只是监听。
//!
//! 同时来的几次锁着 `applied` 一个一个办，每次照这时手里最新的：先后乱了也落在最新的那一个上。

use std::sync::atomic::Ordering;

use super::Running;
use crate::TARGET;
use crate::serve::{Failure, bind};

/// 照桥手里最新的配置换端口（模块的说明）。换了记 `INFO applied`；新端口被占、听不了的不换，记 `WARN`（这就是办了：旧的照旧
/// 开着，没有别的可做，也没有人等结果）。
pub(crate) async fn latest(running: &Running) {
    let mut applied = running.applied.lock().await;
    let wanted = running.current.settings().port;
    if wanted == *applied {
        return;
    }
    let (listener, port) = match bind(wanted).await {
        Ok(bound) => bound,
        Err(Failure::PortInUse(port)) => {
            tracing::warn!(target: TARGET, port, "apply port in use");
            return;
        }
        Err(other) => {
            tracing::warn!(target: TARGET, failure = ?other, "apply failed");
            return;
        }
    };
    running.listen.store(port, Ordering::Relaxed);
    if running.swap.send(listener).is_err() {
        // `serve` 不收了（桥在停）：新的随之放掉，没有别处可交。
        tracing::debug!(target: TARGET, "bridge stopping, listener dropped");
    }
    *applied = wanted;
    tracing::info!(target: TARGET, listen = port, "applied");
}
