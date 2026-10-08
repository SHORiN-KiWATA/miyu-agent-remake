//! 给头看的会话的模型（施工 8-10；思考强度施工 8-18，`docs/blueprint/models.md`「协议」的 `subscribe`、「瞬时事件」的 `model.changed`）：限额，
//! 和会话接下来请求的模型。actor 造会话、载入时，交了新的限额、回合开始重新解析完写一次，和 `Handle` 共用：`subscribe` 照它
//! 答，`model.changed` 照它推，两边说的是同一份。施工 D-3 起还有当前的待办：内核推 `todos.changed` 时跟着换，`subscribe` 照它
//! 回 `todos`。

use miyu_kernel::event::{EffortInUse, Todo};
use miyu_kernel::origin::Model;
use miyu_kernel::session::ContextLimits;

use crate::current::Tally;
use crate::port::ModelPort;
use crate::route::NONE;

/// 给头看的那一份。
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    /// 窗口、压缩线（施工 6-3 补）：向内核要的，和它判到线用的是同一条。
    pub limits: ContextLimits,
    /// 会话接下来请求的模型（施工 8-10）。
    pub next: Next,
    /// 当前的待办（施工 D-3）：造会话、载入时照内核的，内核推 `todos.changed` 时跟着换。
    pub todos: Vec<Todo>,
    /// 这个会话累计的（施工 9-6 上）：造会话时是空的，载入时照整份日志算，之后每落一批盘加上这一批。
    pub tally: Tally,
}

impl Shown {
    /// 造会话、载入时的一份：累计的是空的，载入的由 actor 另照日志算（施工 9-6 上）。
    pub(crate) fn new(limits: ContextLimits, next: Next, todos: Vec<Todo>) -> Shown {
        Shown {
            limits,
            next,
            todos,
            tally: Tally::default(),
        }
    }
}

/// 会话接下来请求的模型（施工 8-10）：`subscribe` 回应的 `model`，`model.changed` 的 `ref`、`endpoint`、`model`。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Next {
    /// 会话的引用：模型或 `@池`。一个都没有的（没配 `models.chat`）没有。
    pub reference: Option<String>,
    /// 接下来发给哪一家的哪个模型。轮换的池（每次都换）、解析不出的没有。
    pub model: Option<Model>,
    /// 接下来那个模型真用的思考强度和从哪来（施工 8-18）：`subscribe` 回应、`model.changed` 的 `effort`。轮换的池、什么都
    /// 不带的没有。
    pub effort: Option<EffortInUse>,
}

impl Next {
    /// 照端口这一刻的：钉着的引用，限额里的模型（是 `none` 的没有），那个模型真用的思考强度（施工 8-18）。
    pub(crate) fn of(port: &dyn ModelPort) -> Next {
        let model = port.limits().model;
        let model = (model.endpoint.as_str() != NONE).then_some(model);
        Next {
            reference: port.reference(),
            effort: model.as_ref().and(port.effort()),
            model,
        }
    }

    /// 两样都没有：`subscribe` 的回应不写 `model`（「一个模型都没有的不写这一格」）。
    pub fn is_empty(&self) -> bool {
        self.reference.is_none() && self.model.is_none()
    }
}
