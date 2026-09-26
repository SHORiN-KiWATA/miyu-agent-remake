//! 内核眼里的工具（`docs/designs/05-内核接口.md` 第六节）：访问类别、参数格式；执行之前的
//! 参数修正；内核替工具写给模型的几句见 [`ToolTexts`]。
//!
//! 工具由软件包提供，内核不内置（`10-自带软件.md` 第一节）。内核只要知道两样：能不能和别的
//! 一起跑（看访问类别），参数长什么样（修正畸形参数）。

mod texts;

use serde_json::{Map, Value};

use crate::raw::RawJson;
use crate::text_enum::text_enum;

pub use texts::{ToolTextSources, ToolTexts};

text_enum!(
    /// 工具的访问类别。权限策略、能不能一起跑、撤销前要不要存档，都看它。请人确认时，请求也
    /// 写明要的是哪一类（`03-事件模型.md` 第三节「确认的事件怎么写」）。
    Access {
        /// 只读：可以和别的只读调用一起跑。
        Read = "read",
        /// 写文件。
        Write = "write",
        /// 执行命令。
        Execute = "execute",
        /// 访问网络。
        Network = "network",
        /// 对外发消息。
        Outbound = "outbound",
    }
);

impl Access {
    /// 要不要写入：写文件的，和不认识的，按最严的算。只读时内核拦下的就是这些
    /// （`02-内核.md` 第六节「权限级别怎么切」「确认怎么走」）。
    pub fn writes(&self) -> bool {
        matches!(self, Access::Write | Access::Other(_))
    }
}

/// 内核要知道的一件工具：访问类别和参数格式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRule {
    /// 访问类别。
    pub access: Access,
    /// 参数的 JSON Schema，和工具面上的一样。
    pub parameters: RawJson,
}

/// 参数不是一个 JSON 对象，修正不了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAnObject;

/// 照参数格式修正模型给的参数原文，返回交给执行的参数：一个 JSON 对象的原文。
///
/// 只动参数格式里声明了类型的顶层参数：被写成字符串的数组、对象、整数、数字、布尔，
/// 能还原成那个类型才换；声明成字符串的，一个字节都不碰。什么都没改的，原文照交。
/// 什么都没写的，当成空对象：有的供应商给没有参数的调用发空字符串。
///
/// # Errors
///
/// 参数不是一个 JSON 对象，返回 [`NotAnObject`]。
pub fn repair(parameters: &RawJson, args: &str) -> Result<String, NotAnObject> {
    if args.trim().is_empty() {
        return Ok("{}".to_string());
    }
    let mut object: Map<String, Value> = serde_json::from_str(args).map_err(|_| NotAnObject)?;
    let schema: Value = match serde_json::from_str(parameters.get()) {
        Ok(schema) => schema,
        Err(_) => return Ok(args.to_string()),
    };
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(args.to_string());
    };
    let mut repaired = false;
    for (name, declared) in properties {
        let Some(kind) = declared.get("type").and_then(Value::as_str) else {
            continue;
        };
        let Some(Value::String(text)) = object.get(name) else {
            continue;
        };
        if let Some(value) = restore(kind, text.trim()) {
            object.insert(name.clone(), value);
            repaired = true;
        }
    }
    if repaired {
        Ok(Value::Object(object).to_string())
    } else {
        Ok(args.to_string())
    }
}

/// 一段字能不能还原成声明的类型。布尔大小写都收：模型发过 Python 风格的 `"False"`。
fn restore(kind: &str, text: &str) -> Option<Value> {
    match kind {
        "array" if text.starts_with('[') => serde_json::from_str::<Value>(text)
            .ok()
            .filter(Value::is_array),
        "object" if text.starts_with('{') => serde_json::from_str::<Value>(text)
            .ok()
            .filter(Value::is_object),
        "integer" => text.parse::<i64>().ok().map(Value::from),
        "number" => text
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .map(Value::from),
        "boolean" => match text.to_ascii_lowercase().as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests;
