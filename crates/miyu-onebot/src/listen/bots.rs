//! 连着的机器人号（`onebot.md` 第一条「怎么走」第 2、10 条）：一个号一条连接，同一个号再连进来，新的顶掉旧的（旧的关掉）；
//! 回话照号找它现在的连接。后台页的 `status`、状态文件照这里说连着哪个号、是哪个实现（施工 O-16、O-18、O-28）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use serde_json::{Map, Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::onebot::Calls;

/// 一条 NapCat 的连接：往它写的队列，它上面在等回应的调用。
#[derive(Debug, Clone)]
pub(crate) struct Link {
    /// 这条连接的序号：断开时只拿掉自己，不拿掉顶掉它的那一条。
    pub(crate) serial: u64,
    /// 往这条连接写的队列。
    pub(crate) out: mpsc::Sender<Message>,
    /// 这条连接上在等回应的调用。
    pub(crate) calls: Arc<Calls>,
    /// 对端是哪个实现：连上以后问 `get_version_info`，回了才有（第 3 条）。
    pub(crate) peer: Arc<OnceLock<Peer>>,
}

/// 对端是哪个实现：`get_version_info` 回的 `app_name`、`app_version`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Peer {
    /// 实现的名字，例如 `NapCat.Onebot`。
    pub(crate) implementation: String,
    /// 实现的版本。
    pub(crate) version: String,
}

/// 号到连接。
#[derive(Debug, Default)]
pub(crate) struct Bots {
    links: Mutex<HashMap<i64, Link>>,
}

impl Bots {
    /// 号 `bot` 现在用 `link`：原来有一条的，交回它（调的一方关掉它）。
    pub(crate) fn insert(&self, bot: i64, link: Link) -> Option<Link> {
        self.lock().insert(bot, link)
    }

    /// 号 `bot` 现在的连接。
    pub(crate) fn get(&self, bot: i64) -> Option<Link> {
        self.lock().get(&bot).cloned()
    }

    /// 连着的号里最小的那一个和它的连接：`status` 只说一个（`onebot.md` 第一条「后台页」第 2 条）。没有连着的是空的。
    pub(crate) fn first(&self) -> Option<(i64, Link)> {
        self.lock()
            .iter()
            .min_by_key(|(bot, _)| **bot)
            .map(|(bot, link)| (*bot, link.clone()))
    }

    /// NapCat 连没连上、是哪个号、哪个实现：连着的号里最小的那一个，`connected`、`self_id`，问到了是哪个实现的再带
    /// `implementation`、`version`；没连着的只有 `connected: false`（`onebot.md` 第一条「后台页」第 2 条、「状态文件」）。
    pub(crate) fn napcat(&self) -> Value {
        let Some((bot, link)) = self.first() else {
            return json!({"connected": false});
        };
        let mut napcat = Map::new();
        napcat.insert("connected".into(), json!(true));
        napcat.insert("self_id".into(), json!(bot.to_string()));
        if let Some(peer) = link.peer.get() {
            napcat.insert("implementation".into(), json!(peer.implementation));
            napcat.insert("version".into(), json!(peer.version));
        }
        Value::Object(napcat)
    }

    /// 序号是 `serial` 的那一条断开了：号 `bot` 现在还是它的，拿掉；已经被顶掉的不动。
    pub(crate) fn remove(&self, bot: i64, serial: u64) {
        let mut links = self.lock();
        if links.get(&bot).is_some_and(|link| link.serial == serial) {
            links.remove(&bot);
        }
    }

    /// 锁里不 `await`、不会崩；真崩了，表照样能用。
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Link>> {
        self.links.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
