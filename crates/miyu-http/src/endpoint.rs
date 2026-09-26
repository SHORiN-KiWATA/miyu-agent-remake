//! 端点：请求发给谁（`15-模型与供应商.md` 第二节）。key 只在发请求时写进头里，打印出来写成 `***`：
//! 密钥永远不进日志（`07-存储.md` 第九节）。

use std::fmt;

/// 一个供应商的地址、key、另配的头。
#[derive(Clone)]
pub struct Endpoint {
    /// 地址，例如 `https://api.deepseek.com`。路径由驱动接在后面。
    pub base_url: String,
    /// key，发请求时写成 `Authorization: Bearer <key>`。
    key: String,
    /// 供应商另配的头，照先后。
    pub headers: Vec<(String, String)>,
}

impl Endpoint {
    /// 地址和 key，没有另配的头。
    pub fn new(base_url: impl Into<String>, key: impl Into<String>) -> Endpoint {
        Endpoint {
            base_url: base_url.into(),
            key: key.into(),
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
    pub(crate) fn key(&self) -> &str {
        &self.key
    }
}

impl fmt::Debug for Endpoint {
    /// key 写成 `***`；另配的头只写名字，值也可能是密钥。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("Endpoint")
            .field("base_url", &self.base_url)
            .field("key", &"***")
            .field("headers", &names)
            .finish()
    }
}
