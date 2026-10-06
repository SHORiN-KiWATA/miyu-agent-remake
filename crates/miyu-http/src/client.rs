//! HTTP 客户端：一个核心一个，连接池里的连接跨请求复用（`05-内核接口.md` 第七节「HTTP 执行器」）。

use std::time::Duration;

/// 连上一个地址最多等多久。连不上是可重试的错，等太久不如早点重试。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// GET 连上一个地址最多等多久（施工 8-7，`models.md`「怎么走」第二条第 3、10 条）：拉目录、拉模型列表都在后台，连不上
/// 早点放弃。
const FETCH_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

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
    build(proxy, CONNECT_TIMEOUT)
}

/// 造一个 GET 用的客户端（[`crate::get()`]，施工 8-7）：同 [`client()`]，连接的时限是 10 秒。
///
/// # Errors
///
/// 同 [`client()`]。
pub fn fetcher(proxy: Proxy) -> reqwest::Result<reqwest::Client> {
    build(proxy, FETCH_CONNECT_TIMEOUT)
}

/// 照连接的时限造。
fn build(proxy: Proxy, connect: Duration) -> reqwest::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .use_rustls_tls()
        .user_agent(concat!("miyu/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(connect)
        // 链接卡片（miyu-net）开了 gzip/brotli/deflate/zstd 这几个 reqwest 特性；cargo 的特性是整个工作区合起来
        // 的，这几个方法不管特性开没开都存在，专门用来保证不被别的包带起来（W-7 补，http.md「客户端」）。不关的话，
        // 这个客户端会自己带上 Accept-Encoding，请求模型那条路的字节就变了。
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd();
    if proxy == Proxy::Off {
        builder = builder.no_proxy();
    }
    builder.build()
}
