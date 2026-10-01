//! 列模型（`docs/blueprint/models.md`「驱动要守的约定」第 3 条、「怎么走」第二条第 10 条，施工 8-7）：OpenAI 兼容的
//! `GET <地址>/models`，回 `{"data":[{"id":…},…]}`。报了窗口的照 `context_window`、`context_length`、`max_context_length`
//! 先有的算（OpenRouter、Ollama、各家中转的写法）。OpenAI 的这一条不分页；分页的几家另写的驱动随它们自己。

use serde::Deserialize;

use crate::Listed;

/// 列模型发到地址后面的这一截。
pub const MODELS_PATH: &str = "/models";

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
            let window = ["context_window", "context_length", "max_context_length"]
                .iter()
                .find_map(|field| model.get(*field)?.as_u64().filter(|window| *window > 0));
            Some(Listed {
                id: id.to_string(),
                window,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests;
