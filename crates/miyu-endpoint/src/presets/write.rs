//! 新建、改、删预设（施工 P-3 中，`docs/blueprint/presets.md`「改」，16 第四、九节）：只写管理员家目录那一层的
//! `<编号>.toml`。改出厂的、系统区的就是建同名覆盖，只写改了的项；不写编号的是新建，编号由核心起（施工 P-3 补）。
//!
//! - 改：照 `config.set` 的 `changes` 那一套（`crate::toml_changes`），注释、顺序、别的字节照原样；改完的一份连同叠好以后
//!   （底、绕圈）一起查，有错整条不收，什么都不写。
//! - 写盘照配置文件的规矩（`miyu_store::config_file`）：顺着链接写、先写临时文件再替换；替换之前发现这一瞬间有人手改了，
//!   从头再来，三次还不行的 `preset_conflict`。
//! - 删：删掉家目录那一层的文件，下面几层还有的回到它们的样子。

use std::io;

use serde::Deserialize;
use serde_json::{Value as Json, json};

use miyu_store::config_file::{self, WriteError};
use miyu_store::human::Human;
use miyu_store::layers::valid;
use miyu_store::presets::{Found, Presets};

use super::{TARGET, describe, presets, told};
use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::toml_changes::{ChangeParams, Wanted, check_expect, edited, wanted, without};

/// 写的时候撞上有人手改，最多重来几次。
const ATTEMPTS: usize = 3;

/// `preset.set` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    #[serde(default)]
    preset: Option<String>,
    changes: Vec<ChangeParams>,
}

/// `preset.set`：照 `changes` 改家目录那一层，交回改完叠好的样子（同 `preset.get`）。不写 `preset` 的是新建（施工 P-3 补）：
/// 编号由核心起，至少要写一项。
pub(crate) async fn set(core: &Core, peer: Peer, params: SetParams) -> Result<Json, Refusal> {
    if params.changes.is_empty() || params.preset.as_deref().is_some_and(|id| !valid(id)) {
        return Err(Refusal::BAD_PARAMS);
    }
    let said = words(core, peer.language).ok();
    let wanted = wanted(params.changes)?;
    if params.preset.is_none() && !wanted.iter().any(Wanted::writes) {
        return Err(Refusal::BAD_PARAMS);
    }
    // 撤掉了的几格不收（施工 P-4 上）：头还在发，说明它没跟上，早报出来好。
    if wanted
        .iter()
        .any(|want| retired().iter().any(|key| key == want.key()))
    {
        return Err(Refusal::BAD_PARAMS);
    }
    let presets = presets(core);
    let found = tokio::task::spawn_blocking(move || match params.preset {
        Some(id) => write(&presets, &id, &wanted, said.as_ref()),
        None => create(&presets, &wanted, said.as_ref()),
    })
    .await
    .map_err(|_| Refusal::INTERNAL)??;
    Ok(describe(core, &found, peer.language))
}

/// 预设里撤掉了的几格的完整的键：P-3 补撤掉的「以谁为底」、P-4 上撤掉的默认人格。读的时候当没写，写的时候去掉。
fn retired() -> [String; 2] {
    [
        format!("preset.{}", miyu_policy::persona::BASE),
        format!("preset.{}", miyu_policy::preset::DEFAULT_PERSONA),
    ]
}

/// 新建：挑一个没用过的编号，照 `wanted` 写一份新文件；写的那一瞬间别处占了这个编号的（文件已经有了），换下一个再来。
fn create(presets: &Presets, wanted: &[Wanted], said: Option<&Human>) -> Result<Found, Refusal> {
    for _ in 0..ATTEMPTS {
        let id = presets.unused_id();
        let file = format!("home {id}.toml");
        let text = edited("", wanted, &file, Refusal::preset_invalid)?;
        let found = presets
            .find_with(&id, &text)
            .map_err(|error| told(&error, said))?;
        match config_file::write(&presets.home_file(&id), text.as_bytes(), None) {
            Ok(()) => {
                tracing::info!(target: TARGET, preset = %id, "preset created");
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

/// 读家目录那一层、查 `expect`、改、查改完的一份、写；撞上有人手改的从头再来。什么都没变的不写，照盘上的找。
fn write(
    presets: &Presets,
    id: &str,
    wanted: &[Wanted],
    said: Option<&Human>,
) -> Result<Found, Refusal> {
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
        check_expect(&text, wanted, "preset_conflict")?;
        let file = format!("home {id}.toml");
        let edited = edited(
            &without(&text, &retired()),
            wanted,
            &file,
            Refusal::preset_invalid,
        )?;
        if edited == text {
            return presets.find(id).map_err(|error| told(&error, said));
        }
        let found = presets
            .find_with(id, &edited)
            .map_err(|error| told(&error, said))?;
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
