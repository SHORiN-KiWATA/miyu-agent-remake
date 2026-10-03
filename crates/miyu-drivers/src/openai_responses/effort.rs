//! 思考强度怎么写（`docs/blueprint/drivers/openai-responses.md`「思考强度」，`models.md`「驱动要守的约定」第 13 条）：接在请求
//! 的最后。
//!
//! - 没有：什么都不加，照供应商的默认；思考不回传、也看不到摘要（图纸「起草时定的」第 12 条）。
//! - 档位：`"reasoning":{"effort":"<档位>","summary":"auto"}`，`"include":["reasoning.encrypted_content"]`：要摘要给人看，要
//!   加密的思考好原样回传（`store` 是假的，服务端不存）。
//! - `off`（目录写 `none` 的那一档）：`"reasoning":{"effort":"none"}`。
//! - `on`：这一家没有开关，不会有；来了什么都不加。

use crate::{EFFORT_OFF, EFFORT_ON};

/// 照这一次的思考强度 `effort`，接在 `body` 后面（不含收尾的 `}`）。
pub(super) fn write(body: &mut Vec<u8>, effort: Option<&str>) {
    match effort {
        None | Some(EFFORT_ON) => {}
        Some(EFFORT_OFF) => body.extend_from_slice(br#","reasoning":{"effort":"none"}"#),
        Some(level) => {
            body.extend_from_slice(br#","reasoning":{"effort":"#);
            super::json(body, level);
            body.extend_from_slice(
                br#","summary":"auto"},"include":["reasoning.encrypted_content"]"#,
            );
        }
    }
}
