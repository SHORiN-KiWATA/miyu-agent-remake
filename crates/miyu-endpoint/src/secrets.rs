//! 密钥（`docs/blueprint/config.md`「协议」`secret.*`、「怎么走」第九条，G9，施工 8-5）：协议上的 `secret.set`、
//! `secret.delete`、`secret.list`，手改密钥文件被看到的，留痕。
//!
//! 只能写、删、列名字，从不交出值：值不进回应、推送、运行日志、报错的话、系统日志（`07-存储.md` 第九节）。`secret.set` 的
//! 参数不进运行日志：端点的 `DEBUG request` 那一行本来只记方法名。
//!
//! 1. `secret.set`：名字不合写法、值去掉前后空白是空的、有控制字符、超过 16 KiB：`bad_params`。拿着配置服务的锁，先把密钥
//!    文件重读一遍（手改过的先当手改记），读不进来的：`config_file_broken`。只改那一行（新建的先写一行开头的注释），照
//!    配置文件的规矩写盘、Unix 上 0600；替换前发现有人手改，从重读重来，最多三次。落了盘记 `secret.changed`、
//!    `INFO secret changed`，换上，再回应 `{"replaced"}`。
//! 2. `secret.delete`：同上；没有这个密钥：`unknown_secret`。回应 `{}`。
//! 3. `secret.list`：设了的，和系统配置、个人设置的最终值里引用了还没设的，照名字排；`used_by` 是引用它的项，照键名排。
//! 4. 手改被看到的（第九条第 3 条）：字节一样的什么都不做；变了的名字每个记一条 `secret.changed`（`via` 是 `file`，`by` 是
//!    内核，没有 `cause`）、`INFO secret changed`；读不进来的照上一次读好的用。不推：密钥的推送随界面（M9）。
//!
//! 写不成的：`internal_error`，记一条 `WARN config not written`，一个字节不动。

mod file;

pub(crate) use file::SecretsFile;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use miyu_config::secret::{self, Secret};
use miyu_kernel::id::CommandId;
use miyu_kernel::origin::{By, Person};
use miyu_store::config_file::{self, WriteError};

use crate::Core;
use crate::config::journal;
use crate::config::methods::{said_at, words};
use crate::config::{Config, TARGET};
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 替换前发现有人手改，最多重来几次（第五条第 6 条）。
const TRIES: usize = 3;

/// `secret.set` 的参数。值只在这里停一下，马上变成 [`Secret`]。
#[derive(Deserialize)]
pub(crate) struct SetParams {
    /// 密钥的名字。
    name: String,
    /// 密钥本身。
    value: String,
}

/// `secret.delete` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct DeleteParams {
    /// 密钥的名字。
    name: String,
}

/// `secret.changed` 的 `body`：名字、做了什么、怎么改的。只记名字，从不记值。
#[derive(Debug, Serialize)]
struct SecretChanged<'a> {
    /// 密钥的名字。
    name: &'a str,
    /// `set` 新设、`replaced` 换掉、`deleted` 删掉。
    action: &'static str,
    /// `set` 经 `secret.set`、`secret.delete`，`file` 手改被看到的。
    via: &'static str,
}

/// 改一个密钥：写入还是删掉。
enum Edit {
    /// 写入或者换掉。
    Set(Secret),
    /// 删掉。
    Delete,
}

/// `secret.set`：写入或者换掉一个密钥，落了盘回 `{"replaced"}`。`cause` 是这条命令的编号，记进日志。
pub(crate) fn set(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    params: SetParams,
) -> Result<Value, Refusal> {
    if !secret::valid_name(&params.name) {
        return Err(Refusal::BAD_PARAMS);
    }
    let value = Secret::new(&params.value).map_err(|_| Refusal::BAD_PARAMS)?;
    let action = change(core, peer, cause, &params.name, Edit::Set(value))?;
    Ok(json!({ "replaced": action == "replaced" }))
}

/// `secret.delete`：删掉一个密钥，落了盘回 `{}`。没有这个密钥：`unknown_secret`。
pub(crate) fn delete(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    params: DeleteParams,
) -> Result<Value, Refusal> {
    change(core, peer, cause, &params.name, Edit::Delete)?;
    Ok(json!({}))
}

/// 改一个密钥：重读、改那一行、写盘，重来最多三次。交回做了什么（`set`、`replaced`、`deleted`）。
fn change(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    name: &str,
    edit: Edit,
) -> Result<&'static str, Refusal> {
    let words = words(core, peer.language)?;
    let Some(header) = miyu_config::Words::sentence(&words, "config/secrets-header", &[]) else {
        tracing::warn!(target: TARGET, error = "no words for config/secrets-header", "config words missing");
        return Err(Refusal::INTERNAL);
    };
    let mut config = core.config();
    for _ in 0..TRIES {
        observe(core, &mut config);
        let file = &config.secrets;
        if file.broken.is_some() {
            let layers = config.layers(None);
            let mut problems = Vec::new();
            for problem in file.problems() {
                problems.push(said_at(
                    &config,
                    &layers,
                    problem,
                    Some((&file.shown, file.last_good)),
                    &words,
                )?);
            }
            return Err(Refusal::config_file_broken(problems));
        }
        let had = file.has(name);
        let (text, action) = match &edit {
            Edit::Set(value) => {
                let base = match file.version {
                    Some(_) => file.text.clone(),
                    None => format!("# {header}\n"),
                };
                let action = if had { "replaced" } else { "set" };
                (secret::set_in(&base, name, value), action)
            }
            Edit::Delete if !had => return Err(Refusal::UNKNOWN_SECRET),
            Edit::Delete => (secret::unset_in(&file.text, name), "deleted"),
        };
        let Ok(text) = text else {
            // 这个名字在文件里写成了别的样子（一张表）：这一行本来就报着错，先把文件改好。
            let layers = config.layers(None);
            let mut problems = Vec::new();
            for problem in file.problems() {
                problems.push(said_at(
                    &config,
                    &layers,
                    problem,
                    Some((&file.shown, file.last_good)),
                    &words,
                )?);
            }
            return Err(Refusal::config_file_broken(problems));
        };
        let bytes = config_file::bytes(&text, file.bom);
        match miyu_store::secrets::write(&file.path, &bytes, file.version.as_deref()) {
            Ok(()) => {
                let written = file.written(text, config_file::version(&bytes));
                config.secrets = written;
                let by = By::Person(Person::new(config.places.account.clone()));
                record(&config, name, action, "set", by, Some(cause));
                core.hub.publish(&config, None);
                return Ok(action);
            }
            Err(WriteError::Changed) => {}
            Err(WriteError::Io(error)) => {
                tracing::warn!(target: TARGET, file = %file.shown, error = %error, "config not written");
                return Err(Refusal::INTERNAL);
            }
        }
    }
    tracing::warn!(target: TARGET, file = %config.secrets.shown, error = "changed while writing", "config not written");
    Err(Refusal::INTERNAL)
}

/// `secret.list`：名字、设没设、谁在用，照名字排。
pub(crate) fn list(core: &Core) -> Value {
    let config = core.config();
    let mut listed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in config.secrets.stored.entries.keys() {
        listed.entry(name.clone()).or_default();
    }
    // 引用了它的真的键：密钥的列表（供应商的几个 key，施工 8-6）里的也算。
    for (name, key) in miyu_config::secret::used(&config.resolved().values()) {
        listed.entry(name).or_default().push(key);
    }
    let secrets: Vec<Value> = listed
        .into_iter()
        .map(|(name, mut used_by)| {
            used_by.sort_unstable();
            used_by.dedup();
            json!({"name": name, "set": config.secrets.has(&name), "used_by": used_by})
        })
        .collect();
    json!({ "secrets": secrets })
}

/// 重读密钥文件（手改被看到的，`secret.*` 写之前）：一样的什么都不做；不一样的换上（读不进来的照上一次读好的用），
/// 变了的名字每个记一条，交给会话、核心（不推）。
pub(crate) fn observe(core: &Core, config: &mut Config) {
    let old = &config.secrets;
    let fresh = SecretsFile::read(&old.path, &old.shown).keeping(old);
    if fresh.same_as(old) {
        return;
    }
    told(&fresh);
    let changes = changes(old, &fresh);
    config.secrets = fresh;
    for (name, action) in &changes {
        record(config, name, action, "file", By::Kernel, None);
    }
    core.hub.publish(config, None);
}

/// 读到的密钥文件组、别人读得到：记一条 `WARN secrets readable by others`，照用，不去改它（第九条第 2 条）。
pub(crate) fn told(file: &SecretsFile) {
    if file.open {
        tracing::warn!(target: TARGET, file = %file.shown, "secrets readable by others");
    }
}

/// 两份之间变了的名字：新设、换掉、删掉，照名字排。
fn changes(old: &SecretsFile, new: &SecretsFile) -> Vec<(String, &'static str)> {
    let (before, after) = (&old.stored.entries, &new.stored.entries);
    let mut names: Vec<&String> = before.keys().chain(after.keys()).collect();
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .filter_map(|name| {
            let action = match (before.get(name), after.get(name)) {
                (None, Some(_)) => "set",
                (Some(was), Some(now)) if was != now => "replaced",
                (Some(_), None) => "deleted",
                _ => return None,
            };
            Some((name.clone(), action))
        })
        .collect()
}

/// 记一条 `secret.changed` 进系统日志，一条 `INFO secret changed`：只有名字。
fn record(
    config: &Config,
    name: &str,
    action: &'static str,
    via: &'static str,
    by: By,
    cause: Option<&CommandId>,
) {
    let body = SecretChanged { name, action, via };
    journal::record(
        &config.places.system_journal,
        &format!("system/{}", miyu_store::journal::FILE),
        crate::sessions::now(),
        by,
        cause,
        "secret.changed",
        &body,
    );
    tracing::info!(target: TARGET, name, action, via, "secret changed");
}

#[cfg(test)]
mod tests;
