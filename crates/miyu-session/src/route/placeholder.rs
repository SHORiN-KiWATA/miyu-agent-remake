//! 给发往 opencode Zen 的请求补占位工具（施工 8-14 补，`docs/blueprint/models.md`「八、opencode Zen」第 2 条）：免费档按客户端
//! 识别，请求体的工具面里要有 `shell` 和 `read`。档案点名了哪几件（`placeholder_tools`），说明是
//! `resources/core/drivers/placeholder-tool.txt` 那一句。
//!
//! 补在统一的请求上、驱动编码之前（`exchange.rs`、`probe.rs` 调），所以三种驱动都成立；有这两件的会话一个字节都不动——补
//! 之前先看名字在不在，缺一件补一件，补完照名字排（`kernel/request.rs` 的规矩）。
//!
//! 她真调了占位的那件：内核的工具规则里没有它（快照的工具面里没有），照没有这件工具处理（`kernel/tools.md`），不会多出权限。

use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Request, ToolSpec};

/// 占位声明的参数格式：空对象，照原样写。
const EMPTY_PARAMETERS: &str = r#"{"type":"object","properties":{}}"#;

/// 工具面里有没有缺的占位（`exchange.rs` 先问它：一件都不缺就不拷一份请求）。
pub(super) fn missing(request: &Request, placeholders: &[(String, String)]) -> bool {
    placeholders
        .iter()
        .any(|(name, _)| !request.tools.iter().any(|tool| &tool.name == name))
}

/// 工具面里缺的占位补上；一件都不缺的原样不动。
pub(super) fn fill(request: &mut Request, placeholders: &[(String, String)]) {
    let mut additions: Vec<ToolSpec> = placeholders
        .iter()
        .filter(|(name, _)| !request.tools.iter().any(|tool| &tool.name == name))
        .map(|(name, description)| ToolSpec {
            name: name.clone(),
            description: description.clone(),
            parameters: serde_json::from_str::<RawJson>(EMPTY_PARAMETERS)
                .expect("写死的参数格式读得进"),
        })
        .collect();
    if additions.is_empty() {
        return;
    }
    request.tools.append(&mut additions);
    request.tools.sort_by(|a, b| a.name.cmp(&b.name));
}

#[cfg(test)]
mod tests;
