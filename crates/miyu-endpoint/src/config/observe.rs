//! 手改被看到的（`docs/blueprint/config.md`「怎么走」第七条第 3 条、第六条第 4 条，施工 8-4）：监视系统配置、个人设置、
//! 信任的记录所在的目录，哪一份变了重读。
//!
//! 1. 字节和上一次读的一样（核心自己写的也走这里）：什么都不做。
//! 2. 不一样：照第二条读、解析、合并。读不进来的照上一次读好的用。这一层变了的项、问题有了变化的：记日志（`via` 是
//!    `file`，`by` 是内核，没有 `cause`）、记一条 `INFO config changed`、推 `config.changed`；都换上新的一份。
//! 3. 文件被删了：这一层变成空的，照样推、记。
//! 4. 信任的记录变了的：每个变了的仓库记一条 `trust.changed`（`via` 是 `file`），换上；不推（项目配置不推）。读不懂的
//!    照上一次读好的用，记一条 `WARN trust not read`。
//!
//! `config.set` 写之前的重读也走 [`observe`]：那一瞬间之前的手改，先当手改推、记，再改。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_config::{Layer, Value};
use miyu_kernel::origin::By;
use miyu_store::watch::{Watch, watch};

use super::file::File;
use super::hub::Pushing;
use super::journal::{self, ConfigChanged, KeyChange, TrustChanged};
use super::push::Via;
use super::{Config, TARGET, trust};
use crate::Core;

/// 一项的改动：键、之前这一层的值、之后的（没写的是空的）。
pub(super) type Difference = (&'static str, Option<Value>, Option<Value>);

impl Core {
    /// 开始监视配置文件（核心读完配置以后，第七条）：交回监视，丢掉就停。系统的监视起不来的退回轮询，记一条
    /// `WARN config watch unavailable`；轮询也起不来的，记同一条，不监视。开始以后先把几份都看一遍：读配置和开始监视之间
    /// 的手改也认得。
    pub fn watch_config(self: &Arc<Self>) -> Option<Watch> {
        let files = self.config().watched();
        let core = Arc::downgrade(self);
        let watching = watch(&files, move |path| {
            if let Some(core) = core.upgrade() {
                core.config_file_changed(path);
            }
        });
        let watching = match watching {
            Ok(watching) => watching,
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "config watch unavailable");
                return None;
            }
        };
        if let Some(error) = watching.unavailable() {
            tracing::warn!(target: TARGET, error = %error, "config watch unavailable");
        }
        for file in &files {
            self.config_file_changed(file);
        }
        Some(watching)
    }

    /// 监视看到 `path` 这一份变了（合并过的）：重读，变了的记日志、推送（第七条第 3 条）。不是配置服务手里的文件的，不理。
    pub fn config_file_changed(&self, path: &Path) {
        let mut config = self.config();
        if path == config.places.trust {
            observe_trust(self, &mut config);
        } else if let Some(layer) = config.layer_at(path) {
            observe(self, &mut config, layer);
        }
    }
}

impl Config {
    /// 要监视的几份：系统配置、个人设置、信任的记录。
    fn watched(&self) -> Vec<PathBuf> {
        vec![
            self.system.path.clone(),
            self.personal.path.clone(),
            self.places.trust.clone(),
        ]
    }

    /// `path` 是哪一层的文件。
    fn layer_at(&self, path: &Path) -> Option<Layer> {
        [&self.system, &self.personal]
            .into_iter()
            .find(|file| file.path == path)
            .map(|file| file.layer)
    }
}

/// 重读能改的一层 `layer`（第七条第 3 条）：和手里的是同一份的，什么都不做；不一样的换上（读不进来的照上一次读好的用），
/// 这一层变了的项、问题有了变化的，记日志、推送。
pub(super) fn observe(core: &Core, config: &mut Config, layer: Layer) {
    let old = config.file(layer);
    let fresh = File::read(config.items(), layer, old.path.clone(), old.shown.clone()).keeping(old);
    if fresh.same_as(old) {
        return;
    }
    let changes = differences(old, &fresh);
    let problems_changed = !old.problems().eq(fresh.problems());
    config.replace(fresh);
    if changes.is_empty() && !problems_changed {
        // 只动了注释、空行：换上了（版本跟着换），不推、不记。
        core.hub.publish(config, None);
        return;
    }
    let keys: Vec<&'static str> = changes.iter().map(|(key, _, _)| *key).collect();
    record(config, layer, Via::File, By::Kernel, None, &changes);
    core.hub.publish(
        config,
        Some(Pushing {
            layer,
            via: Via::File,
            by: None,
            keys,
        }),
    );
}

/// 改动落了盘（或者看到了手改）：记日志，记一条 `INFO config changed`（第六条）。`cause` 是改它的命令，手改的没有。
pub(super) fn record(
    config: &Config,
    layer: Layer,
    via: Via,
    by: By,
    cause: Option<&miyu_kernel::id::CommandId>,
    changes: &[Difference],
) {
    let (journal, journal_shown) = match layer {
        Layer::System => (
            &config.places.system_journal,
            format!("system/{}", miyu_store::journal::FILE),
        ),
        _ => (
            &config.places.account_journal,
            config.places.shown(miyu_store::journal::FILE),
        ),
    };
    let body = ConfigChanged {
        layer: layer.as_str(),
        file: &config.file(layer).shown,
        via: via.as_str(),
        changes: changes
            .iter()
            .map(|(key, old, new)| KeyChange {
                key,
                old: old.as_ref().map(Value::json),
                new: new.as_ref().map(Value::json),
            })
            .collect(),
    };
    journal::record(
        journal,
        &journal_shown,
        crate::sessions::now(),
        by,
        cause,
        "config.changed",
        &body,
    );
    let keys: Vec<&str> = changes.iter().map(|(key, _, _)| *key).collect();
    tracing::info!(
        target: TARGET,
        layer = %layer.as_str(),
        via = %via.as_str(),
        keys = %keys.join(","),
        "config changed"
    );
}

/// 重读信任的记录（第三条第 3 条、第七条第 3 条）：变了的仓库每个记一条 `trust.changed`（`via` 是 `file`），换上。读不懂的
/// 照上一次读好的用，记一条 `WARN trust not read`。
fn observe_trust(core: &Core, config: &mut Config) {
    let fresh = match trust::read(&config.places.trust) {
        Ok(fresh) => fresh,
        Err(error) => {
            tracing::warn!(
                target: TARGET,
                file = %config.places.shown(trust::FILE),
                error = %error,
                "trust not read"
            );
            return;
        }
    };
    if fresh == config.trust {
        return;
    }
    for answer in trust::changed(&config.trust, &fresh) {
        let body = TrustChanged {
            path: &answer.path,
            version: &answer.version,
            trusted: answer.trusted,
            via: Some(Via::File.as_str()),
        };
        journal::record(
            &config.places.account_journal,
            &config.places.shown(miyu_store::journal::FILE),
            crate::sessions::now(),
            By::Kernel,
            None,
            "trust.changed",
            &body,
        );
        tracing::info!(target: TARGET, path = %answer.path, trusted = answer.trusted, "project trust");
    }
    config.trust = fresh;
    core.hub.publish(config, None);
}

/// 这一层真变了的几项：键、之前的值、之后的值（没写的是空的），照键名排。只看这一层算数的项。
pub(super) fn differences(old: &File, new: &File) -> Vec<Difference> {
    let value = |file: &File, key: &str| {
        file.parsed
            .entries
            .get(key)
            .filter(|entry| entry.counts)
            .map(|entry| entry.value.clone())
    };
    let mut keys: Vec<&'static str> = old
        .parsed
        .entries
        .keys()
        .chain(new.parsed.entries.keys())
        .copied()
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter()
        .filter_map(|key| {
            let (before, after) = (value(old, key), value(new, key));
            (before != after).then_some((key, before, after))
        })
        .collect()
}
