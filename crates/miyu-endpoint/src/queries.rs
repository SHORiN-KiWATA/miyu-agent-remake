//! 可选软件包登记的查询（`web-module.md`「在哪」「起草时定的」第 19 条，`mermaid.md`，施工 W-4）：方法名到
//! 怎么答的一张表。核心起来时照编进来的包（cargo 开关 `mermaid`、以后的 `net`）往这张表里登记一行；没编进来的
//! 包，这张表里压根没有它的方法名，照 `_` → `unknown_method` 的老路走，不是端点里写死的 `#[cfg]` 分支。
//!
//! 登记有两种（施工 W-7，`net.md`「怎么走」第 11 条）：[`Queries::register`] 照一条条办，等它答完才读这个连接的下一行；
//! [`Queries::register_background`] 在后台答，这个连接上后面的请求不等它，回应照 `id` 对上（`link.preview` 抓一页要
//! 几秒）。端点照登记分，不认方法名。
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

/// 一个方法怎么答：照一条条办，还是在后台。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Run {
    /// 照一条条办：等它答完才读这个连接的下一行。
    InTurn,
    /// 在后台答：这个连接上后面的请求不等它（施工 W-7）。
    Background,
}

/// 可选软件包登记查询的那张表：方法名 → 怎么答。核心起来时照编进来的包往里登记，一个方法只登记一次
/// （`crates/miyu-core/src/packages.rs`）。
#[derive(Default)]
pub struct Queries {
    handlers: Vec<(&'static str, Handler, Run)>,
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
    pub fn register<F, Fut>(self, method: &'static str, handler: F) -> Queries
    where
        F: Fn(Arc<Core>, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, QueryError>> + Send + 'static,
    {
        self.add(method, handler, Run::InTurn)
    }

    /// 登记 `method`，在后台答（施工 W-7）：端点收到它，交给一个后台任务办，接着读这个连接的下一行；办完了回应照
    /// `id` 对上，直接放进写队列。连接断了，这些任务一起停。
    ///
    /// # Panics
    ///
    /// 同 [`Queries::register`]：同一个方法名登记了不止一次。
    #[must_use]
    pub fn register_background<F, Fut>(self, method: &'static str, handler: F) -> Queries
    where
        F: Fn(Arc<Core>, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, QueryError>> + Send + 'static,
    {
        self.add(method, handler, Run::Background)
    }

    /// 登记一行。
    fn add<F, Fut>(mut self, method: &'static str, handler: F, run: Run) -> Queries
    where
        F: Fn(Arc<Core>, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, QueryError>> + Send + 'static,
    {
        assert!(
            !self.handlers.iter().any(|(name, _, _)| *name == method),
            "`{method}` 登记了不止一次"
        );
        let wrapped: Handler = Arc::new(move |core, params| Box::pin(handler(core, params)));
        self.handlers.push((method, wrapped, run));
        self
    }

    /// `method` 登记过的话，交回它的处理函数；没登记过的交回 `None`——端点照 `unknown_method` 处理
    /// （没编进来的软件包，它的方法压根不在这张表里，就是这个结果）。查表不碰核心的家底，不需要真的
    /// `Core` 就测得到（`tests::an_unregistered_method_is_not_found`）。
    pub(crate) fn get(&self, method: &str) -> Option<Handler> {
        self.handlers
            .iter()
            .find(|(name, _, _)| *name == method)
            .map(|(_, handler, _)| Arc::clone(handler))
    }

    /// `method` 登记成在后台答的话，交回它的处理函数；没登记的、照一条条办的交回 `None`（施工 W-7）。
    pub(crate) fn background(&self, method: &str) -> Option<Handler> {
        self.handlers
            .iter()
            .find(|(name, _, run)| *name == method && *run == Run::Background)
            .map(|(_, handler, _)| Arc::clone(handler))
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

    /// 在后台答的只有照 `register_background` 登记的那几个（施工 W-7）。
    #[test]
    fn only_background_registrations_are_answered_in_the_background() {
        let queries = Queries::new()
            .register("probe.inline", |_core, params| async move { Ok(params) })
            .register_background("probe.slow", |_core, params| async move { Ok(params) });
        assert!(queries.background("probe.slow").is_some());
        assert!(queries.background("probe.inline").is_none());
        assert!(queries.background("probe.other").is_none());
        assert!(queries.get("probe.slow").is_some(), "两种都在表里");
    }

    #[test]
    #[should_panic(expected = "登记了不止一次")]
    fn registering_the_same_method_twice_in_two_ways_panics() {
        let _queries = Queries::new()
            .register("probe.echo", |_core, _params| async move { Ok(json!({})) })
            .register_background("probe.echo", |_core, _params| async move { Ok(json!({})) });
    }

    #[test]
    #[should_panic(expected = "登记了不止一次")]
    fn registering_the_same_method_twice_panics() {
        let _queries = Queries::new()
            .register("probe.echo", |_core, _params| async move { Ok(json!({})) })
            .register("probe.echo", |_core, _params| async move { Ok(json!({})) });
    }
}
