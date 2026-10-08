//! 新建、改、删预设（施工 P-3 中，`docs/blueprint/presets.md`「改」，16 第四、九节）：只写管理员家目录那一层的
//! `<编号>.toml`。改出厂的、系统区的就是建同名覆盖，只写改了的项；对还没有的编号写就是新建。
//!
//! - 改：照 `config.set` 的 `changes` 那一套，一项项在原来的字上改（`miyu_config::edit::apply`），注释、顺序、别的字节照原样；
//!   改完的一份连同叠好以后（底、绕圈）一起查，有错整条不收，什么都不写。
//! - 写盘照配置文件的规矩（`miyu_store::config_file`）：顺着链接写、先写临时文件再替换；替换之前发现这一瞬间有人手改了，
//!   从头再来，三次还不行的 `preset_conflict`。
//! - 删：删掉家目录那一层的文件，下面几层还有的回到它们的样子。

use std::collections::BTreeSet;
use std::io;

use serde::Deserialize;
use serde_json::{Map, Value as Json, json};

use miyu_config::Value;
use miyu_config::edit::{self, Change};
use miyu_config::plain;
use miyu_store::config_file::{self, WriteError};
use miyu_store::layers::valid;
use miyu_store::presets::{Found, Presets};

use super::{TARGET, describe, presets, refusal};
use crate::Core;
use crate::config::set::same;
use crate::refusal::Refusal;

/// 写的时候撞上有人手改，最多重来几次。
const ATTEMPTS: usize = 3;

/// `preset.set` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    preset: String,
    changes: Vec<ChangeParams>,
}

/// `changes` 的一项：`value`、`unset` 正好写一个。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeParams {
    key: String,
    #[serde(default)]
    value: Option<Json>,
    #[serde(default)]
    unset: Option<bool>,
    #[serde(default)]
    expect: Option<Map<String, Json>>,
}

/// 查过参数的一项：改成的值（`None` 是删掉），和 `expect`（外面一层是写没写，里面是「这一项现在应当没写」）。
struct Wanted {
    key: String,
    value: Option<Value>,
    expect: Option<Option<Json>>,
}

/// `preset.set`：照 `changes` 改家目录那一层，交回改完叠好的样子（同 `preset.get`）。
pub(crate) async fn set(core: &Core, params: SetParams) -> Result<Json, Refusal> {
    if !valid(&params.preset) {
        return Err(Refusal::BAD_PARAMS);
    }
    let wanted = wanted(params.changes)?;
    let presets = presets(core);
    let id = params.preset;
    let found = tokio::task::spawn_blocking(move || write(&presets, &id, &wanted))
        .await
        .map_err(|_| Refusal::INTERNAL)??;
    Ok(describe(core, &found))
}

/// `preset.delete` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeleteParams {
    preset: String,
}

/// `preset.delete`：删掉家目录那一层的文件，交回 `{"remains"}`：下面几层还有没有。家目录那一层本来就没有的
/// `nothing_to_delete`。
pub(crate) async fn delete(core: &Core, params: DeleteParams) -> Result<Json, Refusal> {
    if !valid(&params.preset) {
        return Err(Refusal::BAD_PARAMS);
    }
    let presets = presets(core);
    let id = params.preset;
    tokio::task::spawn_blocking(move || {
        match std::fs::remove_file(presets.home_file(&id)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(Refusal::NOTHING_TO_DELETE);
            }
            Err(error) => {
                tracing::warn!(target: TARGET, kind = ?error.kind(), "preset not deleted");
                return Err(Refusal::INTERNAL);
            }
        }
        tracing::info!(target: TARGET, preset = %id, "preset deleted");
        Ok(json!({"remains": presets.exists(&id)}))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}

/// 查参数：至少一项，同一个键不写两次，`value`、`unset` 正好一个，值是字、开关、数，`expect` 是 `{"value": …}` 或 `{}`。
fn wanted(changes: Vec<ChangeParams>) -> Result<Vec<Wanted>, Refusal> {
    if changes.is_empty() {
        return Err(Refusal::BAD_PARAMS);
    }
    let mut keys = BTreeSet::new();
    changes
        .into_iter()
        .map(|change| {
            if !keys.insert(change.key.clone()) {
                return Err(Refusal::BAD_PARAMS);
            }
            let value = match (change.value, change.unset) {
                (Some(value), None) => Some(plain::from_json(&value).ok_or(Refusal::BAD_PARAMS)?),
                (None, Some(true)) => None,
                _ => return Err(Refusal::BAD_PARAMS),
            };
            let expect = match change.expect {
                None => None,
                Some(expect) if expect.is_empty() => Some(None),
                Some(mut expect) if expect.len() == 1 => {
                    Some(Some(expect.remove("value").ok_or(Refusal::BAD_PARAMS)?))
                }
                Some(_) => return Err(Refusal::BAD_PARAMS),
            };
            Ok(Wanted {
                key: change.key,
                value,
                expect,
            })
        })
        .collect()
}

/// 读家目录那一层、查 `expect`、改、查改完的一份、写；撞上有人手改的从头再来。什么都没变的不写，照盘上的找。
fn write(presets: &Presets, id: &str, wanted: &[Wanted]) -> Result<Found, Refusal> {
    let path = presets.home_file(id);
    for _ in 0..ATTEMPTS {
        let read = config_file::read(&path).map_err(|error| {
            tracing::warn!(target: TARGET, error = %error, "preset unreadable");
            Refusal::INTERNAL
        })?;
        let (text, bom, version) = match read {
            Some(read) => (read.text, read.bom, Some(read.version)),
            None => (String::new(), false, None),
        };
        for want in wanted {
            if let Some(expected) = &want.expect {
                let now = plain::json_at(&text, &want.key);
                if !same(now.as_ref(), expected.as_ref()) {
                    let shown = now.map_or_else(|| json!({}), |value| json!({ "value": value }));
                    return Err(Refusal::conflict("preset_conflict", Some(shown)));
                }
            }
        }
        let edited = edited(id, &text, wanted)?;
        if edited == text {
            return presets.find(id).map_err(|error| refusal(&error));
        }
        let found = presets
            .find_with(id, &edited)
            .map_err(|error| refusal(&error))?;
        match config_file::write(&path, &config_file::bytes(&edited, bom), version.as_deref()) {
            Ok(()) => {
                tracing::info!(target: TARGET, preset = %id, "preset saved");
                return Ok(found);
            }
            Err(WriteError::Changed) => {}
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "preset not saved");
                return Err(Refusal::INTERNAL);
            }
        }
    }
    Err(Refusal::conflict("preset_conflict", None))
}

/// 在 `text` 上一项项改：这一层本来就是这个值的不动（写法不同的也不动，例如单引号）；本来就没写又要删的，`edit::apply` 自己
/// 不动。放不进去的（那一组写成了别的东西）`preset_invalid`。
fn edited(id: &str, text: &str, wanted: &[Wanted]) -> Result<String, Refusal> {
    let mut edited = text.to_string();
    for want in wanted {
        let now = plain::json_at(&edited, &want.key);
        let change = match &want.value {
            Some(value) if now.as_ref() == Some(&value.json()) => continue,
            Some(value) => Change::Set(&want.key, value),
            None => Change::Unset(&want.key),
        };
        edited = edit::apply(&edited, change).map_err(|blocked| {
            Refusal::preset_invalid(format!("home {id}.toml: cannot write {}", blocked.0))
        })?;
    }
    Ok(edited)
}
