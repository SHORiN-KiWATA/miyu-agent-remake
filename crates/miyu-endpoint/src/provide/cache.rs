//! 登记缓存（施工 O-2 中，`docs/blueprint/providers.md`「登记缓存」）：`provide` 成了，这个包的登记写进
//! `state/providers/<包>.json`；核心起来拉起扩展以前、开扩展时，照缓存先登记进目录，连接还没来，调到的暂时不可用。不缓存的话，
//! 核心重启以后、扩展重新登记以前跑的那一轮会把它的工具换掉，登记了又换回来。派生的数据：一样的不写；写不成、读不成、照它
//! 登记不上的记一行 `WARN`，等扩展自己登记。

use std::path::PathBuf;

use miyu_store::generated;

use super::{ProvideParams, register};
use crate::Core;

const TARGET: &str = "miyu::endpoint";

/// 包 `package` 的缓存在哪。
fn path(core: &Core, package: &str) -> PathBuf {
    core.root
        .state()
        .join("providers")
        .join(format!("{package}.json"))
}

/// 记下包 `package` 这一次登记的原文 `kept`（`provide` 的参数）。
pub(super) fn write(core: &Core, package: &str, kept: &[u8]) {
    let path = path(core, package);
    if let Err(error) = generated::write(&path, kept) {
        tracing::warn!(target: TARGET, package, file = %path.display(), error = %error, "provided tools not cached");
    }
}

/// 照缓存先登记包 `package` 的工具：核心起来拉起扩展以前、开扩展时调。没有缓存的什么都不做。
pub(crate) fn restore(core: &Core, package: &str) {
    let path = path(core, package);
    let text = match std::fs::read(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            tracing::warn!(target: TARGET, package, file = %path.display(), error = %error, "provided tools cache unreadable");
            return;
        }
    };
    let restored = serde_json::from_slice::<ProvideParams>(&text)
        .map_err(|error| error.to_string())
        .and_then(|params| {
            register(core, package, params.tools).map_err(|refusal| format!("{refusal:?}"))
        });
    match restored {
        Ok(count) => {
            tracing::info!(target: TARGET, package, tools = count, "provided tools restored")
        }
        Err(error) => {
            tracing::warn!(target: TARGET, package, file = %path.display(), error = error.as_str(), "provided tools cache not used");
        }
    }
}
