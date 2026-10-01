//! 端点：请求发给谁（`15-模型与供应商.md` 第二节）。key 只在发请求时照驱动的认证头写进头里（施工 8-6），打印出来写成
//! `***`：密钥永远不进日志（`07-存储.md` 第九节）。没有 key 的（本机的服务）不带认证头。

use std::fmt;

/// 一个供应商的地址、key、另配的头。
#[derive(Clone)]
pub struct Endpoint {
    /// 地址，例如 `https://api.deepseek.com`。路径由驱动接在后面。
    pub base_url: String,
    /// key，发请求时照驱动写成认证头（[`miyu_drivers::Driver::auth`]）；没有的不带。
    key: Option<String>,
    /// 供应商另配的头，照先后。
    pub headers: Vec<(String, String)>,
}

impl Endpoint {
    /// 地址和 key，没有另配的头。
    pub fn new(base_url: impl Into<String>, key: impl Into<String>) -> Endpoint {
        Endpoint {
            base_url: base_url.into(),
            key: Some(key.into()),
            headers: Vec::new(),
        }
    }

    /// 只有地址，没有 key（本机的服务，施工 8-6）：不带认证头。
    pub fn keyless(base_url: impl Into<String>) -> Endpoint {
        Endpoint {
            base_url: base_url.into(),
            key: None,
            headers: Vec::new(),
        }
    }

    /// 另配一个头。
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Endpoint {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// key：只给发请求的那一处用。
    pub(crate) fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }
}

impl fmt::Debug for Endpoint {
    /// 地址只写主机名，路径和参数里可能有 key（施工 4-9 再补三下）；key 写成 `***`；另配的头只写名字，值也可能是
    /// 密钥。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("Endpoint")
            .field("host", &host(&self.base_url))
            .field("key", &self.key.as_ref().map(|_| "***"))
            .field("headers", &names)
            .finish()
    }
}

/// 地址里的主机名，日志、调试输出只写它：路径和参数里可能有 key。读不出来的写 `?`。
pub(crate) fn host(base_url: &str) -> String {
    reqwest::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "?".to_string())
}
