//! HTTP 客户端：一个核心一个，连接池里的连接跨请求复用（`05-内核接口.md` 第七节「HTTP 执行器」）。

use std::time::Duration;

/// 连上一个地址最多等多久。连不上是可重试的错，等太久不如早点重试。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 走不走代理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proxy {
    /// 照环境变量 `HTTPS_PROXY`、`HTTP_PROXY`、`NO_PROXY` 这些，平时用。
    FromEnvironment,
    /// 不走代理：测试连本机的假服务器时用，开发机上设了代理也不走。
    Off,
}

/// 造一个客户端：TLS 用 rustls，证书认系统的和 webpki 自带的两份；`User-Agent` 是 `miyu/<版本>`。
///
/// # Errors
///
/// 造不出来：TLS 初始化失败这类，照原样交回。
pub fn client(proxy: Proxy) -> reqwest::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .use_rustls_tls()
        .user_agent(concat!("miyu/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT);
    if proxy == Proxy::Off {
        builder = builder.no_proxy();
    }
    builder.build()
}
