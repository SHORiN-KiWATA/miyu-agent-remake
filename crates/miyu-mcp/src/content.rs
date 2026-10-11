//! 列出来的工具、调用回来的内容（`docs/blueprint/mcp.md`「对外的样子」「怎么走」第四条）：照 MCP 的写法读成几种，认不出的
//! 原样留着。换成 Miyu 的工具规格、结果块是核心那一边的事。

use std::time::Duration;

use serde_json::Value;

/// 一件工具。
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// 服务给的名字，原样。
    pub name: String,
    /// 给人看的名字，没写的没有。
    pub title: Option<String>,
    /// 说明，没写的没有。
    pub description: Option<String>,
    /// 参数的 JSON Schema，原样。
    pub input_schema: Value,
    /// 注解：服务自己报的，只用来少问人。
    pub annotations: Annotations,
    /// 服务给的这一项原文：缓存照它写。
    pub raw: Value,
}

/// 工具的注解（MCP 的 `ToolAnnotations`）：都是服务自己说的，没写的是空的。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Annotations {
    /// 只读：不改任何东西。
    pub read_only: Option<bool>,
    /// 会删、会覆盖。
    pub destructive: Option<bool>,
    /// 同样的参数调几次结果一样。
    pub idempotent: Option<bool>,
    /// 碰外面的世界（联网这类）。
    pub open_world: Option<bool>,
}

/// `tools/list` 列全了的。
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    /// 认得出的工具，照服务给的先后。
    pub tools: Vec<Tool>,
    /// 认不出来、没收的几项：说哪一项哪里不对。
    pub skipped: Vec<String>,
    /// 新时代的服务说这份目录多久内不用再问（`ttlMs`）；没说的、旧时代的没有。
    pub fresh_for: Option<Duration>,
}

/// 一次调用回来的。
#[derive(Debug, Clone, PartialEq)]
pub struct Called {
    /// 内容，照先后。
    pub content: Vec<Content>,
    /// 结构化的结果（`structuredContent`），没有的是空的。
    pub structured: Option<Value>,
    /// 工具自己说出错了（`isError`）。
    pub is_error: bool,
}

/// 结果里的一项内容。
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    /// 文字。
    Text(String),
    /// 图片：base64 的数据、类型。
    Image {
        /// base64。
        data: String,
        /// 类型，例如 `image/png`。
        mime: String,
    },
    /// 声音：base64 的数据、类型。
    Audio {
        /// base64。
        data: String,
        /// 类型。
        mime: String,
    },
    /// 一条链接（`resource_link`）：地址、名字。
    Link {
        /// 地址。
        uri: String,
        /// 名字，没写的没有。
        name: Option<String>,
    },
    /// 嵌进来的资源（`resource`）：地址、类型、文字或者 base64 的数据。
    Resource {
        /// 地址。
        uri: String,
        /// 类型，没写的没有。
        mime: Option<String>,
        /// 文字的有。
        text: Option<String>,
        /// 二进制的有，base64。
        blob: Option<String>,
    },
    /// 认不出来的：原样。
    Other(Value),
}

impl Tool {
    /// 读 `tools/list` 里的一项：要有名字、参数的格式是对象。
    ///
    /// # Errors
    ///
    /// 说哪里不对。
    pub fn read(raw: &Value) -> Result<Tool, String> {
        let text = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_string);
        let Some(name) = text("name").filter(|name| !name.is_empty()) else {
            return Err(format!("a tool without a name: {raw}"));
        };
        let input_schema = match raw.get("inputSchema") {
            Some(schema @ Value::Object(_)) => schema.clone(),
            _ => return Err(format!("tool {name}: inputSchema is not an object")),
        };
        let hints = raw.get("annotations");
        let hint = |key: &str| {
            hints
                .and_then(|hints| hints.get(key))
                .and_then(Value::as_bool)
        };
        Ok(Tool {
            title: text("title"),
            description: text("description"),
            input_schema,
            annotations: Annotations {
                read_only: hint("readOnlyHint"),
                destructive: hint("destructiveHint"),
                idempotent: hint("idempotentHint"),
                open_world: hint("openWorldHint"),
            },
            raw: raw.clone(),
            name,
        })
    }
}

impl Called {
    /// 读 `tools/call` 的结果：`content` 不是数组的当空的。
    pub(crate) fn read(result: &Value) -> Called {
        Called {
            content: result
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(Content::read)
                .collect(),
            structured: result
                .get("structuredContent")
                .filter(|value| !value.is_null())
                .cloned(),
            is_error: result.get("isError").and_then(Value::as_bool) == Some(true),
        }
    }
}

impl Content {
    /// 读一项：照 `type` 认，缺了要的格子的当认不出来。
    fn read(raw: &Value) -> Content {
        let text =
            |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        let read = match raw.get("type").and_then(Value::as_str) {
            Some("text") => text(raw, "text").map(Content::Text),
            Some("image") => text(raw, "data")
                .zip(text(raw, "mimeType"))
                .map(|(data, mime)| Content::Image { data, mime }),
            Some("audio") => text(raw, "data")
                .zip(text(raw, "mimeType"))
                .map(|(data, mime)| Content::Audio { data, mime }),
            Some("resource_link") => text(raw, "uri").map(|uri| Content::Link {
                uri,
                name: text(raw, "name"),
            }),
            Some("resource") => raw.get("resource").and_then(|resource| {
                text(resource, "uri").map(|uri| Content::Resource {
                    uri,
                    mime: text(resource, "mimeType"),
                    text: text(resource, "text"),
                    blob: text(resource, "blob"),
                })
            }),
            _ => None,
        };
        read.unwrap_or_else(|| Content::Other(raw.clone()))
    }
}

#[cfg(test)]
mod tests;
