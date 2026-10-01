//! 可选软件包登记的查询（`web-module.md`「在哪」「起草时定的」第 19 条，`mermaid.md`，施工 W-4）：方法名到
//! 怎么答的一张表。核心起来时照编进来的包（cargo 开关 `mermaid`、以后的 `net`）往这张表里登记一行；没编进来的
//! 包，这张表里压根没有它的方法名，照 `_` → `unknown_method` 的老路走，不是端点里写死的 `#[cfg]` 分支。
//!
//! 登记的方法只用得上这几种拒绝（[`QueryError`]），不认得 JSON-RPC 的错误码：参数不对、核心自己出了问题、或者
//! 一个带着原因码（可以附一格 `data`）的 Miyu 拒绝。真正的错误码、`message` 由 `crate::refusal::Refusal`
//! 兜底翻译（`crate::refusal` 对 `miyu-core` 之类的外部 crate 不公开）。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

use crate::Core;

/// 一次查询的回应：拿到参数、算出结果，或者按 [`QueryError`] 拒绝。
pub type Reply = Pin<Box<dyn Future<Output = Result<Value, QueryError>> + Send>>;

/// 登记的方法会拒绝的几种。
#[derive(Debug, Clone)]
pub enum QueryError {
    /// 参数读不成、类型不对。
    BadParams,
    /// 核心自己出了问题：详情记运行日志，不进回应。
    Internal,
    /// 一个具体的原因码（协议上照旧是 `-32010`），没有 `data`。
    Reason(&'static str),
    /// 同上，`data` 里多一格：`(字段名, 值)`。
    ReasonWithDetail(&'static str, &'static str, Value),
}

pub(crate) type Handler = Arc<dyn Fn(Arc<Core>, Value) -> Reply + Send + Sync>;

/// 可选软件包登记查询的那张表：方法名 → 怎么答。核心起来时照编进来的包往里登记，一个方法只登记一次
/// （`crates/miyu-core/src/packages.rs`）。
#[derive(Default)]
pub struct Queries {
    handlers: Vec<(&'static str, Handler)>,
}

impl Queries {
    /// 空表：谁都没登记——没编进来任何可选软件包的核心就是这张表。
    pub fn new() -> Queries {
        Queries::default()
    }

    /// 登记 `method`：往后端点收到这个方法名，就交给 `handler` 办。
    ///
    /// # Panics
    ///
    /// 同一个方法名登记了不止一次：这是接线时的 bug，不是运行时会出现的状况，照别处的「不该走到的状态」
    /// 用 `expect` 当场说清。
    #[must_use]
    pub fn register<F, Fut>(mut self, method: &'static str, handler: F) -> Queries
    where
        F: Fn(Arc<Core>, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, QueryError>> + Send + 'static,
    {
        assert!(
            !self.handlers.iter().any(|(name, _)| *name == method),
            "`{method}` 登记了不止一次"
        );
        let wrapped: Handler = Arc::new(move |core, params| Box::pin(handler(core, params)));
        self.handlers.push((method, wrapped));
        self
    }

    /// `method` 登记过的话，交回它的处理函数；没登记过的交回 `None`——端点照 `unknown_method` 处理
    /// （没编进来的软件包，它的方法压根不在这张表里，就是这个结果）。查表不碰核心的家底，不需要真的
    /// `Core` 就测得到（`tests::an_unregistered_method_is_not_found`）。
    pub(crate) fn get(&self, method: &str) -> Option<Handler> {
        self.handlers
            .iter()
            .find(|(name, _)| *name == method)
            .map(|(_, handler)| Arc::clone(handler))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// 没登记过的方法交回 `None`：没编进来某个可选软件包时，核心起来就不会往这张表里登记它的方法名，
    /// 端点看到的就是这个结果，照 `unknown_method` 处理（`web-module.md`「起草时定的」第 19 条）。
    #[test]
    fn an_unregistered_method_is_not_found() {
        let queries = Queries::new();
        assert!(queries.get("mermaid.render").is_none());
        assert!(queries.get("anything").is_none());
    }

    #[test]
    fn a_registered_method_is_found_by_its_exact_name() {
        let queries =
            Queries::new().register("probe.echo", |_core, params| async move { Ok(params) });
        assert!(queries.get("probe.echo").is_some());
        assert!(queries.get("probe.ech").is_none(), "名字要正好对上");
        assert!(queries.get("probe.echo2").is_none());
    }

    #[test]
    #[should_panic(expected = "登记了不止一次")]
    fn registering_the_same_method_twice_panics() {
        let _queries = Queries::new()
            .register("probe.echo", |_core, _params| async move { Ok(json!({})) })
            .register("probe.echo", |_core, _params| async move { Ok(json!({})) });
    }
}
