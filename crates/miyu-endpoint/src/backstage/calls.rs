//! `package.methods`、`package.call`、`method.call`（施工 F-6 中，`package-pages.md`「`package.methods`、`package.call`、
//! `method.call`」）：扩展登记它的后台页要调的方法，头调，核心反向转给扩展、交回它回的。登记不缓存：连接断了，调到的回
//! `not_running`。

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;
use crate::reverse::{Gone, Peer};

/// 不写时限的等多久。
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// 时限能写的范围，毫秒（同工具的）。
const TIMEOUT_RANGE: std::ops::RangeInclusive<u64> = 1000..=600_000;

/// 哪个包登记了哪些方法、由哪个连接答。
#[derive(Default)]
pub(crate) struct Methods {
    table: Mutex<HashMap<String, Registered>>,
}

/// 一个包登记的。
struct Registered {
    peer: Peer,
    methods: BTreeMap<String, Duration>,
}

/// `package.methods` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterParams {
    methods: Vec<MethodParams>,
}

/// 登记的一个方法。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MethodParams {
    name: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

/// `package.call` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CallParams {
    package: String,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

/// `package.methods`：换掉这个包上一次登记的，交回 `{"methods": 个数}`。不是核心拉起的扩展的连接 `not_an_extension`；
/// 名字写法不对、重复、时限不在范围里的 `bad_params`，什么都不换。
pub(crate) fn register(
    core: &Core,
    caller: &Caller,
    params: RegisterParams,
) -> Result<Value, Refusal> {
    let Some(package) = caller.package.as_deref() else {
        return Err(Refusal::NOT_AN_EXTENSION);
    };
    let mut methods = BTreeMap::new();
    for method in params.methods {
        let timeout = method.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        if !valid(&method.name)
            || methods.contains_key(&method.name)
            || !TIMEOUT_RANGE.contains(&timeout)
        {
            return Err(Refusal::BAD_PARAMS);
        }
        methods.insert(method.name, Duration::from_millis(timeout));
    }
    let count = methods.len();
    core.backstage
        .table
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            package.to_string(),
            Registered {
                peer: caller.reverse.clone(),
                methods,
            },
        );
    tracing::info!(target: "miyu::endpoint", package, methods = count, "page methods registered");
    Ok(json!({ "methods": count }))
}

/// `package.call`：照登记的转给扩展，回什么交回什么。
pub(crate) async fn call(core: &Core, params: CallParams) -> Result<Value, Refusal> {
    let installed = core
        .packages()
        .iter()
        .any(|found| found.id == params.package && found.read.is_ok());
    if !installed {
        return Err(Refusal::UNKNOWN_PACKAGE);
    }
    let (peer, timeout) = {
        let table = core
            .backstage
            .table
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let registered = table.get(&params.package).ok_or(Refusal::NOT_RUNNING)?;
        let timeout = *registered
            .methods
            .get(&params.method)
            .ok_or_else(|| Refusal::unregistered(&params.method))?;
        (registered.peer.clone(), timeout)
    };
    let request =
        json!({"method": params.method, "params": params.params.unwrap_or_else(|| json!({}))});
    match tokio::time::timeout(timeout, peer.call("method.call", request)).await {
        Err(_) => Err(Refusal::METHOD_TIMEOUT),
        Ok(Err(Gone)) => Err(Refusal::NOT_RUNNING),
        Ok(Ok(Ok(result))) => Ok(result),
        Ok(Ok(Err(error))) => Err(Refusal::method_failed(
            error
                .get("message")
                .and_then(Value::as_str)
                .map_or_else(|| error.to_string(), str::to_string),
            error.get("code").cloned(),
        )),
    }
}

/// 方法名：小写字母开头，小写字母、数字、`_`、`.`、`-`，1 到 64 个。
pub(super) fn valid(name: &str) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
}

#[cfg(test)]
mod tests;
