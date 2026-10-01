//! 还没写进配置的一家（`docs/blueprint/models.md`「协议」`provider.test` 的 `candidate`，施工 8-11）：照写进配置以后的样子
//! 写成一份最终值，交给和会话的路由同一个 [`crate::provider::provider`] 推，试的和写进去以后真用的不会不一样。
//!
//! - 编号是 `catalog`（`miyu setup` 写配置时用的就是它），没写的是 [`CANDIDATE`]。`catalog` 不另写成一格：编号就是它。
//! - `keys` 总写（没有 key 的是空的列表）：这一家因此算配好了。
//! - `{value}` 的 key 只在这一次的内存里：最终值里写成 [`value_key`] 这个引用，取值的一方只认它。它不合密钥名字的写法，
//!   和密钥文件里真的名字撞不上。

use std::borrow::Cow;

use miyu_config::secret::Reference;
use miyu_config::{Value, Values};

/// 没写 `catalog` 的候选的编号。
pub const CANDIDATE: &str = "candidate";

/// `{value}` 的 key 在最终值里写成的引用：名字里有括号，不合密钥名字的写法。
pub fn value_key() -> Reference {
    Reference::Secret("(value)".to_string())
}

/// 还没写进配置的一家：每一格都可以不写，没写的照档案、目录推。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Candidate {
    /// 驱动的写法。
    pub driver: Option<String>,
    /// 写死的地址。
    pub base_url: Option<String>,
    /// key 的引用；`{value}` 的写成 [`value_key`]。
    pub key: Option<Reference>,
    /// 目录、档案里的编号。
    pub catalog: Option<String>,
}

impl Candidate {
    /// 编号：`catalog`，没写的是 [`CANDIDATE`]。
    pub fn id(&self) -> String {
        self.catalog
            .clone()
            .unwrap_or_else(|| CANDIDATE.to_string())
    }

    /// 写成一份最终值：只有 `providers.<编号>.*` 这几格。
    pub fn values(&self) -> Values {
        let at = |name: &str| format!("providers.{}.{name}", self.id());
        let text = |value: &str| Value::Text(Cow::Owned(value.to_string()));
        let mut values = Values::default();
        if let Some(driver) = &self.driver {
            values.set(&at("driver"), text(driver));
        }
        if let Some(base_url) = &self.base_url {
            values.set(&at("base_url"), text(base_url));
        }
        let keys = self.key.iter().cloned().map(Value::Secret).collect();
        values.set(&at("keys"), Value::List(keys));
        values
    }
}
