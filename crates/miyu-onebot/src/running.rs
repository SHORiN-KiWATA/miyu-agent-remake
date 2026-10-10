//! 桥跑着的样子（`onebot.md` 第一条「在哪」、「施工时定的」第 168 条，施工 O-28 下从 WebUI 的 `Web` 挪出来）：实际听的 NapCat
//! 端口、连着的机器人号、桥手里最新的配置、桥自己的数。后台页的 `status`（`crate::core::methods`）、状态文件
//! （`crate::status_file`）照它说，推来的端口变化照它换（[`rebind`]）。

pub(crate) mod rebind;

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};

use serde_json::{Map, Value, json};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::current::Current;
use crate::listen::bots::Bots;
use crate::onebot::PLATFORM;
use crate::tuning::Tuning;

/// 跑着的桥，几样共用。
pub(crate) struct Running {
    /// NapCat 连进来的端口，实际听的那一个（设的是 0 的，系统挑的那一个）。推来的换了的照换了的。
    pub(crate) listen: AtomicU16,
    /// 桥自己的数：`status` 的 `path` 照它的 `paths`。
    pub(crate) tuning: Tuning,
    /// 连着的机器人号：`status`、状态文件照它说 NapCat 连没连上。
    pub(crate) bots: Arc<Bots>,
    /// 桥手里最新的配置（和 NapCat 的监听共用，施工 O-20）。
    pub(crate) current: Arc<Current>,
    /// 上一次照的 NapCat 端口，照配置里写的（写 0 的就是 0）：换端口照它看变了没有。锁着它办：同时来的几次一个一个办。
    pub(crate) applied: tokio::sync::Mutex<u16>,
    /// 换端口开好的新监听交给 `serve` 换上。
    pub(crate) swap: mpsc::UnboundedSender<TcpListener>,
}

impl Running {
    /// 后台页的 `status` 照它答的几格（施工 O-28 上，`onebot.md` 第一条「后台页」第 2 条）：`napcat`、`listen`、`token`、
    /// `platform`，照桥手里最新的，不读盘。
    ///
    /// - `napcat`：连着的号里最小的那一个，问到了是哪个实现的才带 `implementation`、`version`；没连着的只有
    ///   `connected: false`（`Bots::napcat`，状态文件也照它）。
    /// - `token`：桥手里有令牌的 `set`，没有的 `none`。没写引用、引用取不到，核心都不交，分不出（「施工时定的」第 41 条）。
    /// - `platform`：桥的平台名（[`PLATFORM`]），白名单成员页照它拼 `qq:<号>`（第 151 条）。
    pub(crate) fn status(&self) -> Map<String, Value> {
        let token = match self.current.token() {
            Some(_) => "set",
            None => "none",
        };
        let mut status = Map::new();
        status.insert("napcat".into(), self.bots.napcat());
        status.insert("listen".into(), json!(self.listen.load(Ordering::Relaxed)));
        status.insert("token".into(), json!(token));
        status.insert("platform".into(), json!(PLATFORM));
        status
    }
}
