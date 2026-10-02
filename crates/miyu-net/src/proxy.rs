//! 这一跳走不走代理（`net.md`「怎么走」第 5 条，项目主人 2026-10-01 定）：和请求模型一样照环境变量
//! `HTTPS_PROXY`、`HTTP_PROXY`、`ALL_PROXY`、`NO_PROXY`（小写的也认）。判用 `hyper-util` 的 `Matcher`：reqwest 自己
//! 照环境变量走代理用的就是它，`NO_PROXY` 的读法和请求模型一字不差（「起草时定的」第 1 条）。判出来要走的，交回
//! 交给 reqwest 的代理（`Proxy::all`，地址和认证照 `Matcher` 交回的，第 2 条）。

use hyper_util::client::proxy::matcher::Matcher;
use reqwest::Url;

/// 环境变量里的代理：核心第一次调 `link.preview` 时读一次。
pub(crate) struct Proxies {
    matcher: Matcher,
}

/// 这一跳交给哪个代理。
#[derive(Debug, Clone)]
pub(crate) struct Via {
    /// 代理的地址，例如 `http://127.0.0.1:<端口>/`：客户端照它复用。
    pub(crate) uri: String,
    /// 代理要的认证（地址里写了用户名、密码的）：照原样交给 reqwest。
    auth: Option<reqwest::header::HeaderValue>,
}

impl Proxies {
    /// 照这时的环境变量。
    pub(crate) fn from_env() -> Proxies {
        Proxies {
            matcher: Matcher::from_env(),
        }
    }

    /// 测试用：代理照 `proxy`（`None` 是不走代理），`NO_PROXY` 照 `no_proxy`，不读环境变量。
    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn given(proxy: Option<&str>, no_proxy: &str) -> Proxies {
        let mut builder = Matcher::builder().no(no_proxy);
        if let Some(proxy) = proxy {
            builder = builder.all(proxy);
        }
        Proxies {
            matcher: builder.build(),
        }
    }

    /// `url` 这一跳交给哪个代理；不走代理的（没设，或者在 `NO_PROXY` 里）是 `None`。
    pub(crate) fn via(&self, url: &Url) -> Option<Via> {
        let uri: http::Uri = url.as_str().parse().ok()?;
        let found = self.matcher.intercept(&uri)?;
        Some(Via {
            uri: found.uri().to_string(),
            auth: found.basic_auth().cloned(),
        })
    }
}

impl Via {
    /// 交给 reqwest 的代理：这个客户端发的都经它。
    ///
    /// # Errors
    ///
    /// reqwest 读不懂这个地址（`Matcher` 已经读过一遍，不该走到）。
    pub(crate) fn proxy(&self) -> reqwest::Result<reqwest::Proxy> {
        let proxy = reqwest::Proxy::all(self.uri.as_str())?;
        Ok(match &self.auth {
            Some(auth) => proxy.custom_http_auth(auth.clone()),
            None => proxy,
        })
    }
}
