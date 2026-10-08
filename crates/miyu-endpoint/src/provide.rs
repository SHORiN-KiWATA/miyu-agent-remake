//! `provide {tools}`（施工 O-2 上，`docs/blueprint/providers.md`）：核心拉起的扩展把它现在提供的全部工具登记进核心的工具目录，
//! 归它的包，换掉它上一次登记的；记下这个包现在由这个连接提供（[`Provided`]）。连接断了，工具照旧留在目录里，被调到时回
//! 「暂时不可用」。

mod remote;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_tool::{Problem, Spec, Tool, Venues};

use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;
use crate::reverse::Peer;

use remote::RemoteTool;

/// 哪个包现在由哪个连接提供。
#[derive(Default)]
pub(crate) struct Provided {
    peers: Mutex<HashMap<String, Peer>>,
}

impl Provided {
    /// 现在提供包 `package` 的连接；没有的是没有。
    pub(crate) fn peer(&self, package: &str) -> Option<Peer> {
        self.peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(package)
            .cloned()
    }

    /// 记下包 `package` 由 `peer` 提供，换掉以前的。
    fn register(&self, package: &str, peer: Peer) {
        self.peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(package.to_string(), peer);
    }
}

/// `provide` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProvideParams {
    tools: Vec<ToolParams>,
}

/// 一件工具的规格。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolParams {
    name: String,
    description: String,
    input_schema: Value,
    access: String,
    venues: Vec<String>,
}

/// `provide`：见模块的说明。
pub(crate) async fn provide(
    core: &Arc<Core>,
    caller: &Caller,
    params: ProvideParams,
) -> Result<Value, Refusal> {
    let Some(package) = caller.package.as_deref() else {
        return Err(Refusal::NOT_A_PROVIDER);
    };
    let unavailable = core
        .resources
        .core_texts()
        .map(|texts| texts.tool_results.unavailable)
        .map_err(|_| Refusal::INTERNAL)?;
    let count = params.tools.len();
    let mut tools: Vec<Arc<dyn Tool>> = Vec::with_capacity(count);
    for tool in params.tools {
        let (spec, venues) = checked(tool)?;
        tools.push(Arc::new(RemoteTool::new(
            spec,
            venues,
            package,
            Arc::clone(&core.provided),
            unavailable.clone(),
        )));
    }
    let replaced = core
        .tools()
        .replacing(package, tools)
        .map_err(|error| bad_tool(&error.tool, problem(error.problem)))?;
    core.set_tools(replaced);
    core.provided.register(package, caller.reverse.clone());
    tracing::info!(target: "miyu::endpoint", package, tools = count, "provided");
    Ok(json!({ "tools": count }))
}

/// 查一件的访问类别和给哪种会话；名字、参数格式、撞名由目录查（[`miyu_tool::Catalog::replacing`]）。
fn checked(tool: ToolParams) -> Result<(Spec, Venues), Refusal> {
    let access = serde_json::from_value::<Access>(Value::String(tool.access.clone()))
        .ok()
        .filter(|access| !matches!(access, Access::Other(_)))
        .ok_or_else(|| bad_tool(&tool.name, "access"))?;
    let mut venues = Venues::default();
    for venue in &tool.venues {
        match venue.as_str() {
            "local" => venues.local = true,
            "private" => venues.private = true,
            "group" => venues.group = true,
            _ => return Err(bad_tool(&tool.name, "venues")),
        }
    }
    if venues == Venues::default() {
        return Err(bad_tool(&tool.name, "venues"));
    }
    let parameters: RawJson = serde_json::from_str(&tool.input_schema.to_string())
        .map_err(|_| bad_tool(&tool.name, "parameters"))?;
    Ok((
        Spec {
            name: tool.name,
            description: tool.description,
            parameters,
            access,
        },
        venues,
    ))
}

/// 目录查出来的那一条，写成协议上的字。
fn problem(problem: Problem) -> &'static str {
    match problem {
        Problem::Duplicate => "duplicate",
        Problem::Name => "name",
        Problem::Parameters => "parameters",
    }
}

/// `bad_tool`：哪一件、哪一条。
fn bad_tool(tool: &str, problem: &str) -> Refusal {
    Refusal::bad_tool(tool, problem)
}

#[cfg(test)]
mod tests;
