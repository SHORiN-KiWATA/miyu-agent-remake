//! 列模型（`docs/blueprint/drivers/anthropic.md`「对外的样子」，`models.md`「驱动要守的约定」第 3 条）：
//! `GET <地址>/models?limit=1000`，回 `{"data":[{"id":…,"max_input_tokens":…},…],"has_more":…}`。窗口是 `max_input_tokens`。
//! `has_more` 不看：一页 1000 个，官方的模型远没有这么多（图纸「起草时定的」第 12 条）。

use serde::Deserialize;

use crate::Listed;

/// 列模型发到地址后面的这一截。
pub const MODELS_PATH: &str = "/models?limit=1000";

/// 回应的样子：只读用得上的格。
#[derive(Deserialize)]
struct Page {
    data: Vec<serde_json::Value>,
}

/// 读列模型的回应：每个模型的名字和报了的窗口。名字不是字、是空的跳过；窗口不是正整数的当没报。
///
/// # Errors
///
/// 不是 JSON，或者没有 `data` 数组：原话说是哪一种。
pub fn parse_models(bytes: &[u8]) -> Result<Vec<Listed>, String> {
    let page: Page = serde_json::from_slice(bytes)
        .map_err(|error| format!("model list not readable: {error}"))?;
    Ok(page
        .data
        .iter()
        .filter_map(|model| {
            let id = model.get("id")?.as_str().filter(|id| !id.is_empty())?;
            let window = model
                .get("max_input_tokens")
                .and_then(serde_json::Value::as_u64)
                .filter(|window| *window > 0);
            Some(Listed {
                id: id.to_string(),
                window,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests;
