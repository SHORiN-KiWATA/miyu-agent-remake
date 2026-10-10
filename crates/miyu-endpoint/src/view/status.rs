//! 会话状态（施工 9-8 补上，`docs/blueprint/view.md`「会话状态」）：投影算得出的那一半（在跑没跑、在等什么、正在做什么、
//! 上下文用了多少、速度、冷却、任务表），拼上向会话要的（用量、模型、权限、待办、工作区）和订阅时定下的（人格、预设）。
//! 视图流订阅的回应带一份，之后变了推整份 `view.status`。

use serde_json::{Value, json};

use miyu_session::{Current, Handle};
use miyu_view::Status;

use crate::Core;

/// 订阅时定下、之后不变的：会话用哪个人格、哪个预设（照日志第一条 `session.created`，以前的日志没有的是 `None`）。
#[derive(Debug, Clone, Default)]
pub(crate) struct Fixed {
    pub(crate) persona: Option<Value>,
    pub(crate) preset: Option<Value>,
}

/// 整份会话状态。`current` 是向会话 actor 要的那一份（拿不到的不写那几格）。
pub(crate) fn whole(
    core: &Core,
    handle: &Handle,
    projected: Status,
    current: Option<&Current>,
    fixed: &Fixed,
) -> Value {
    let used = projected.used;
    let mut status = json!(projected);
    if let Some(status) = status.as_object_mut() {
        status.remove("used");
    }
    let limits = handle.limits();
    let mut context = json!(limits);
    if let Some(used) = used {
        context["used"] = json!(used);
    }
    status["context"] = context;
    if let Some(model) = crate::models::next(&handle.next()) {
        status["model"] = model;
    }
    status["todos"] = json!(handle.todos());
    if let Some(current) = current {
        status["usage"] = crate::usage::tallied(core, &current.tally);
        status["permission"] = json!(current.permission);
        status["workspace"] = json!({"cwd": current.cwd, "dirs": current.dirs});
    }
    if let Some(persona) = &fixed.persona {
        status["persona"] = persona.clone();
    }
    if let Some(preset) = &fixed.preset {
        status["preset"] = preset.clone();
    }
    status
}
