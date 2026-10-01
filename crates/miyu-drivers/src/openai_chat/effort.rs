//! 思考强度怎么写（施工 8-18，`docs/blueprint/models.md`「驱动要守的约定」第 13 条）：接在请求的最后。
//!
//! - 没有思考强度：什么都不加，请求和以前一个字节不差。
//! - 档位（目录里的名字）：`"reasoning_effort":"<档位>"`。
//! - `off`：档案写了开关的（[`Compat::toggle`]），写开关的「关」，例如 DeepSeek 的 `"thinking":{"type":"disabled"}`；没写的
//!   写 `"reasoning_effort":"none"`，OpenAI 兼容的接口关思考就是这么写的。
//! - `on`：写了开关的写开关的「开」；没写的没有别的说法，什么都不加，照供应商的默认。

use super::{Compat, json};
use crate::{EFFORT_OFF, EFFORT_ON};

/// 照这一次的思考强度 `effort` 和这一家的开关 `compat`，接在 `body` 后面（不含收尾的 `}`）。
pub(super) fn write(body: &mut Vec<u8>, effort: Option<&str>, compat: &Compat) {
    let Some(level) = effort else {
        return;
    };
    match (level, &compat.toggle) {
        (EFFORT_OFF, Some(toggle)) => field(body, &toggle.field, &toggle.off),
        (EFFORT_ON, Some(toggle)) => field(body, &toggle.field, &toggle.on),
        (EFFORT_ON, None) => {}
        (EFFORT_OFF, None) => field(body, "reasoning_effort", &"none"),
        (level, _) => field(body, "reasoning_effort", &level),
    }
}

/// 写一个顶层字段：`,"<name>":<value>`。
fn field(body: &mut Vec<u8>, name: &str, value: &(impl serde::Serialize + ?Sized)) {
    body.push(b',');
    json(body, name);
    body.push(b':');
    json(body, value);
}
