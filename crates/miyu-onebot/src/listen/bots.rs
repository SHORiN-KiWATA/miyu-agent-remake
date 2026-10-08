//! 连着的机器人号（`onebot.md` 第一条「怎么走」第 2、10 条）：一个号一条连接，同一个号再连进来，新的顶掉旧的（旧的关掉）；
//! 回话照号找它现在的连接。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

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
