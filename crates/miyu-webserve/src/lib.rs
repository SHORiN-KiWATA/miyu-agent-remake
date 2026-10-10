//! 只听本机的网页端口共用的底子（`docs/blueprint/webserve.md`，施工 O-16）：网页软件 `miyu-web`（`web-ui.md`）用它；QQ 桥原来
//! 的 WebUI 随施工 O-28 下去掉；照终端的样子连核心的 `open` 施工 S-1 挪进了 `miyu-client`（设计 32 第一节）。
//!
//! - [`respond`]：回应的正文、一律带的安全响应头。
//! - [`pages`]：页面文件：路径不出页面目录、类型照扩展名、响应头。
//! - [`Site`]：Host、Origin 认的三种写法；`/ws` 要的数据根、怎么拉起核心、连着的怎么数。
//! - [`ws`]：`/ws` 原样转给核心，一帧一行。
//!
//! 函数都从 `miyu-web` 原样搬来，不改名、不改行为（`webserve.md`「搬家表」）。听端口、单实例、空闲退出、`/media` 这些
//! 各家不一样的，留在各家。分层照 `01-架构.md` 第九节第 3 层：只依赖 `miyu-ipc`、`miyu-store`，不依赖任何一个头。

pub mod pages;
pub mod respond;
#[cfg(test)]
mod tests;
pub mod ws;

use std::sync::Arc;

use miyu_store::root::DataRoot;

/// 运行日志的目标。搬来的代码照原样记在 `miyu::web` 下：网页软件的 `web.log` 一个字不变；桥的运行日志里这几行的来源也是
/// `web`，说的是它网页那一头。
const TARGET: &str = "miyu::web";

use miyu_client::CoreCommand;

/// 一个跑着的网页端口，各个连接共用：Host、Origin 照它的端口核对（`web-ui.md`「怎么走」第一条第 4、7 款），`/ws` 照它
/// 连核心、数忙（[`ws::accept`]）。网页软件、QQ 桥各实现一份，只交出自己手里的几样。
pub trait Site: Send + Sync + 'static {
    /// 连着一个 WebSocket 时拿着的：放下了算走了。不数的是 `()`。
    type Busy: Send + 'static;

    /// 数据根：照它找核心的套接字。
    fn root(&self) -> &DataRoot;

    /// 实际听的端口（设的是 0 的，系统挑的那一个）。
    fn port(&self) -> u16;

    /// 核心没在跑时怎么拉起来。
    fn core(&self) -> &CoreCommand;

    /// 数上一个连着的 WebSocket。
    fn busy(self: &Arc<Self>) -> Self::Busy;

    /// Host、Origin 认的三种写法（不带协议）。
    fn hosts(&self) -> [String; 3] {
        let port = self.port();
        [
            format!("127.0.0.1:{port}"),
            format!("localhost:{port}"),
            format!("[::1]:{port}"),
        ]
    }

    /// Host 对不对：只认三种写法，`localhost` 不分大小写。别的网站把自己的域名解析到回环地址也进不来（DNS rebinding）。
    fn host_allowed(&self, host: Option<&str>) -> bool {
        host.is_some_and(|host| {
            self.hosts()
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host))
        })
    }
}
