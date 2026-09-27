//! 协议端点（`docs/designs/04-核心协议.md`，施工 3-8 上）：头和核心之间说的话。
//!
//! 一个连接上说 JSON-RPC 2.0，一行一条消息。先握手：协议的主版本，本机令牌；之后能造会话、说话、
//! 打断，订阅会话的事件流（施工 3-8 中）。会话表照编号找会话，这次运行里没在跑的，从磁盘载入。只认字节流，不管它从哪来：本机套接字
//! （施工 3-8 下）、命名管道、以后的 WebSocket，都把连接交给 [`serve`]。
//!
//! - [`Core`]：核心的家底：数据根、资源目录、给会话造请求模型的端口、管理员、本机令牌、会话表；
//! - [`serve`]：和一个连接说话，直到它关了；
//! - [`run`]：在本机的监听器上一个个接连接，每个交给 [`serve`]；
//! - [`Core::idle`]：没有连接、也没有在跑的回合，核心据此空闲退出（施工 3-9 上）。

mod connection;
mod hello;
mod listen;
mod methods;
mod refusal;
mod sessions;
mod subscriptions;
mod wire;

pub use connection::serve;
pub use listen::run;

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use miyu_kernel::id::AccountId;
use miyu_session::Models;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use sessions::Sessions;

/// 核心的家底：一个核心一份，各个连接一起用。
pub struct Core {
    /// 数据根。
    root: DataRoot,
    /// 资源目录：造会话时读人格。
    resources: ResourceRoot,
    /// 给会话造请求模型的端口。
    models: Arc<dyn Models>,
    /// 管理员：本机连上来的都是他（`06-多用户与身份.md` 第二节）。
    admin: AccountId,
    /// 本机令牌：本机连接握手时要出示（`04-核心协议.md` 第四节）。
    token: String,
    /// 会话表。
    sessions: Sessions,
    /// 连着几个连接：`serve` 开始时加一，走的时候减一（施工 3-9 上）。
    connections: AtomicUsize,
}

impl Core {
    /// 一份家底：会话表是空的，会话用到时再载入。
    pub fn new(
        root: DataRoot,
        resources: ResourceRoot,
        models: Arc<dyn Models>,
        admin: AccountId,
        token: String,
    ) -> Core {
        Core {
            root,
            resources,
            models,
            admin,
            token,
            sessions: Sessions::default(),
            connections: AtomicUsize::new(0),
        }
    }

    /// 连着几个连接。
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::Acquire)
    }

    /// 空闲：没有连接，也没有在跑的回合。核心看它决定能不能空闲退出（`12-进程形态与分发.md` 第二节，
    /// 施工 3-9 上）。
    pub async fn idle(&self) -> bool {
        self.connections() == 0 && !self.sessions.busy().await
    }

    /// 有计划地停下全部在跑的会话：核心收到停的信号时。跑到一半的回合记成「重启了」，下次载入接着干。
    pub async fn stop_sessions(&self) {
        self.sessions.stop_all().await;
    }
}

/// 连着的一个连接：数着，走的时候减掉，连接的任务被叫停了也减。
pub(crate) struct Connected(Arc<Core>);

impl Connected {
    /// 数上一个。
    pub(crate) fn new(core: Arc<Core>) -> Connected {
        core.connections.fetch_add(1, Ordering::AcqRel);
        Connected(core)
    }
}

impl Drop for Connected {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 令牌不打出来。
impl fmt::Debug for Core {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Core")
            .field("root", &self.root)
            .field("admin", &self.admin)
            .finish_non_exhaustive()
    }
}
