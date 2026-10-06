//! 思考强度怎么写（`docs/blueprint/drivers/anthropic.md`「思考强度」，`models.md`「驱动要守的约定」第 13 条）：接在请求的最后。
//!
//! - 没有：什么都不加，照供应商的默认（Opus 5 这一代默认会思考，4.7、4.8 默认不思考）。
//! - 档位：`"thinking":{"type":"adaptive","display":"summarized"}`，`"output_config":{"effort":"<档位>"}`。`display` 写
//!   `summarized`：4.7 起默认不给思考的字，写了才看得到思考的摘要，不多花钱。
//! - `off`：`"thinking":{"type":"disabled"}`。开关是接口自带的，不用档案写。
//! - `on`：`"thinking":{"type":"adaptive","display":"summarized"}`。
//!
//! 温度（施工 8-22，[`temperature`]）：开着思考只能用默认的温度，带了别的整个请求被拒。档位、`on` 的不带温度；`off`、没有的
//! 照交来的带。

use crate::{Call, EFFORT_OFF, EFFORT_ON};

/// 开着思考的写法。
const ADAPTIVE: &str = r#","thinking":{"type":"adaptive","display":"summarized"}"#;

/// 照这一次的思考强度 `effort`，接在 `body` 后面（不含收尾的 `}`）。
pub(super) fn write(body: &mut Vec<u8>, effort: Option<&str>) {
    match effort {
        None => {}
        Some(EFFORT_OFF) => body.extend_from_slice(br#","thinking":{"type":"disabled"}"#),
        Some(EFFORT_ON) => body.extend_from_slice(ADAPTIVE.as_bytes()),
        Some(level) => {
            body.extend_from_slice(ADAPTIVE.as_bytes());
            body.extend_from_slice(br#","output_config":{"effort":"#);
            super::json(body, level);
            body.push(b'}');
        }
    }
}

/// 这一次带不带温度：思考开着（档位、`on`）的不带，别的照 `call.temperature`。
pub(super) fn temperature(call: &Call) -> Option<f64> {
    match call.effort.as_deref() {
        None | Some(EFFORT_OFF) => call.temperature,
        Some(_) => None,
    }
}
