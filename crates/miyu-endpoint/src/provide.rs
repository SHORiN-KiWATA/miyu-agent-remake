//! `provide {tools}`（施工 O-2 上，`docs/blueprint/providers.md`）：核心拉起的扩展把它现在提供的全部工具登记进核心的工具目录，
//! 归它的包，换掉它上一次登记的；记下这个包现在由这个连接提供（[`Provided`]）。连接断了，工具照旧留在目录里，被调到时回
//! 「暂时不可用」。登记缓存在磁盘上，核心起来、开扩展时先照缓存登记；关掉扩展，它的工具出目录（施工 O-2 中，[`cache`]）。

mod cache;
mod remote;

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_tool::{Problem, Spec, Tool, Venues};

use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;
use crate::reverse::Peer;

use remote::{RemoteTool, Stores, Texts};

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

/// `provide` 的参数，也是登记缓存的写法（施工 O-2 中）。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProvideParams {
    tools: Vec<ToolParams>,
}

/// 一件工具的规格。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolParams {
    name: String,
    description: String,
    input_schema: Value,
    access: String,
    venues: Vec<String>,
    /// 等它答多久（施工 O-2 下）：毫秒，[`TIMEOUT_MS`] 以内，不写是 [`DEFAULT_TIMEOUT_MS`]。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
}

/// 不写时等多久：一分钟。
const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// 能写的时限：一秒到十分钟。
const TIMEOUT_MS: std::ops::RangeInclusive<u64> = 1_000..=600_000;

/// 查过的一件：规格、给哪种会话、等它多久。
struct Checked {
    spec: Spec,
    venues: Venues,
    timeout: Duration,
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
    let kept = serde_json::to_vec(&params).map_err(|_| Refusal::INTERNAL)?;
    let count = register(core, package, params.tools)?;
    core.provided.register(package, caller.reverse.clone());
    cache::write(core, package, &kept);
    tracing::info!(target: "miyu::endpoint", package, tools = count, "provided");
    Ok(json!({ "tools": count }))
}

/// 照规格 `tools` 换掉包 `package` 在目录里的工具，交回几件：`provide`、读缓存共用。两处同时换的一个接一个（[`miyu_tool::Shelf`]）。
fn register(core: &Core, package: &str, tools: Vec<ToolParams>) -> Result<usize, Refusal> {
    let texts = Texts {
        unavailable: core
            .resources
            .core_texts()
            .map(|texts| texts.tool_results.unavailable)
            .map_err(|_| Refusal::INTERNAL)?,
        timed_out: core
            .resources
            .tool_timed_out()
            .map_err(|_| Refusal::INTERNAL)?,
    };
    let count = tools.len();
    let listed = listed(core, package);
    let mut built: Vec<Arc<dyn Tool>> = Vec::with_capacity(count);
    for tool in tools {
        // 写了 `[features]` 的包登记的工具要归它列的某个功能（施工 T-2）：归不上的预设开关不了它。
        if listed
            .as_ref()
            .is_some_and(|listed| !listed.contains(&tool.name))
        {
            return Err(bad_tool(&tool.name, "feature"));
        }
        built.push(Arc::new(RemoteTool::new(
            checked(tool)?,
            package,
            Arc::clone(&core.provided),
            texts.clone(),
            Stores {
                root: core.root.clone(),
                admin: core.admin.clone(),
            },
        )));
    }
    core.tools
        .replace(|catalog| catalog.replacing(package, built))
        .map_err(|error| bad_tool(&error.tool, problem(error.problem)))?;
    Ok(count)
}

/// 包 `package` 的工具归得上的：清单写了 `[features]`、又不是只有一个功能的，是它列的工具名（施工 T-2，归法同
/// `miyu_policy::features::Features::of_tool`）。没写的整个包算一个功能、只写了一个功能的都归它、清单读不成的，没有：不查。
fn listed(core: &Core, package: &str) -> Option<BTreeSet<String>> {
    let packages = core.packages();
    let manifest = packages
        .iter()
        .find(|one| one.id == package)?
        .read
        .as_ref()
        .ok()?;
    let features = manifest
        .features
        .as_ref()
        .filter(|features| features.len() != 1)?;
    Some(
        features
            .iter()
            .flat_map(|feature| feature.tools.iter().cloned())
            .collect(),
    )
}

/// 关掉扩展（施工 O-2 中）：包 `package` 的工具出目录，开着的会话下一个回合换掉；缓存留着，再开时照它先登记。
pub(crate) fn withdraw(core: &Core, package: &str) {
    let withdrawn = core
        .tools
        .replace(|catalog| catalog.replacing(package, Vec::new()));
    if let Err(error) = withdrawn {
        // 拿掉不会撞名：只是把这个包的去掉。
        tracing::error!(target: "miyu::endpoint", package, error = %error, "tools not withdrawn");
    }
}

pub(crate) use cache::restore;

/// 查一件的访问类别、给哪种会话、时限；名字、参数格式、撞名由目录查（[`miyu_tool::Catalog::replacing`]）。
fn checked(tool: ToolParams) -> Result<Checked, Refusal> {
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
    let timeout = tool.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !TIMEOUT_MS.contains(&timeout) {
        return Err(bad_tool(&tool.name, "timeout"));
    }
    let parameters: RawJson = serde_json::from_str(&tool.input_schema.to_string())
        .map_err(|_| bad_tool(&tool.name, "parameters"))?;
    Ok(Checked {
        spec: Spec {
            name: tool.name,
            description: tool.description,
            parameters,
            access,
        },
        venues,
        timeout: Duration::from_millis(timeout),
    })
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
