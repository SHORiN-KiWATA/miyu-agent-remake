//! 协议上的 `config.trust`（`docs/blueprint/config.md`「协议」、「怎么走」第三条第 3 条，G3，施工 8-3）：信任、不信任一份
//! 项目配置。
//!
//! 1. 缺了格、类型不对：`bad_params`。这个目录找不到项目配置：`no_project_config`。
//! 2. `version` 和这份文件现在的版本对不上（人看过以后它又变了）：`config_conflict`，`data.version` 是现在的版本。人信任的
//!    只能是他看过的那一份。
//! 3. 收下的：先重读 `trust.toml`（手改过的在新的字上记），记下这个回答，照改配置的规矩写盘（顺着链接、临时文件、替换前再读、
//!    有人手改就重来，最多三次）；换上新的记录，记账号日志 `trust.changed`、运行日志 `INFO project trust`，再回应。
//! 4. 不推送：项目配置不推（第三条第 5 条）。下一个会话、下一句话、开着的会话的下一个回合照新的（施工 8-4）。
//!
//! `trust.toml` 读不懂（手改坏了）、写不成的：`internal_error`，记一条 `WARN config not written`，一个字节不动。

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_kernel::id::CommandId;
use miyu_kernel::origin::{By, Person};
use miyu_store::config_file::{self, WriteError};

use super::journal::{self, TrustChanged};
use super::methods::words;
use super::project;
use super::trust::{self, Answer};
use super::{Config, TARGET};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 替换前发现有人手改，最多重来几次。
const TRIES: usize = 3;

/// `config.trust` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct TrustParams {
    /// 照这个目录找项目配置。
    cwd: String,
    /// 人看过的那一份的版本。
    version: String,
    /// 信不信任。
    trust: bool,
}

/// `config.trust`：记下了交回 `{"file", "trusted"}`。`cause` 是这条命令的编号，记进日志。
pub(crate) fn trust(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    params: TrustParams,
) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let Some(header) = words.sentence("config/trust-header", &[]) else {
        tracing::warn!(target: TARGET, error = "no words for config/trust-header", "config words missing");
        return Err(Refusal::INTERNAL);
    };
    let mut config = core.config();
    let Some(project) = config.project(&params.cwd) else {
        return Err(Refusal::NO_PROJECT_CONFIG);
    };
    if project.file.version.as_deref() != Some(params.version.as_str()) {
        return Err(Refusal::config_conflict_version(project.file.version));
    }
    let shown = project::shown(&project.repo, config.home.as_deref());
    let answer = Answer {
        repo: &project.repo,
        shown: &shown,
        version: &params.version,
        trusted: params.trust,
    };
    let file = config.places.shown(trust::FILE);
    for _ in 0..TRIES {
        match write(&config, &header, &answer) {
            Ok(records) => {
                config.trust = records;
                done(&config, cause, &answer);
                // 不推，只交给会话：下一个回合照新的信任读项目配置（施工 8-4）。
                core.hub.publish(&config, None);
                return Ok(json!({"file": project.file.shown, "trusted": params.trust}));
            }
            Err(WriteError::Changed) => {}
            Err(WriteError::Io(error)) => {
                tracing::warn!(target: TARGET, file = %file, error = %error, "config not written");
                return Err(Refusal::INTERNAL);
            }
        }
    }
    Err(Refusal::config_conflict_version(project.file.version))
}

/// 重读 `trust.toml`、记下回答、写盘，交回写进去的记录。
fn write(
    config: &Config,
    header: &str,
    answer: &Answer<'_>,
) -> Result<Vec<trust::Record>, WriteError> {
    let path: &Path = &config.places.trust;
    let unreadable = |why: String| WriteError::Io(std::io::Error::other(why));
    let read = config_file::read(path).map_err(|error| unreadable(error.to_string()))?;
    let text = trust::recorded(
        read.as_ref().map(|read| read.text.as_str()),
        header,
        answer,
        config.home.as_deref(),
    )
    .map_err(unreadable)?;
    let bom = read.as_ref().is_some_and(|read| read.bom);
    let version = read.map(|read| read.version);
    config_file::write(path, &config_file::bytes(&text, bom), version.as_deref())?;
    trust::records(&text).map_err(unreadable)
}

/// 记下了：账号日志 `trust.changed`，运行日志 `INFO project trust`。
fn done(config: &Config, cause: &CommandId, answer: &Answer<'_>) {
    let body = TrustChanged {
        path: answer.shown,
        version: answer.version,
        trusted: answer.trusted,
        via: None,
    };
    let by = By::Person(Person {
        account: config.places.account.clone(),
    });
    journal::record(
        &config.places.account_journal,
        &config.places.shown(miyu_store::journal::FILE),
        crate::sessions::now(),
        by,
        Some(cause),
        "trust.changed",
        &body,
    );
    tracing::info!(target: TARGET, path = %answer.shown, trusted = answer.trusted, "project trust");
}
