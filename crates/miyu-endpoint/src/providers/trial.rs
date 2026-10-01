//! `provider.test`（`docs/blueprint/models.md`「协议」、「怎么走」第七条第 4 条，施工 8-11）：试一家配好了的供应商
//! （`provider`），或者还没写进配置的一家（`candidate`），会真的花一点额度。
//!
//! 1. `provider`、`candidate` 不是正好写一个、`candidate` 不是那个形状（`key` 不是三种之一、多了别的格）、`model` 是空的：
//!    `bad_params`。`provider` 不是配好了的：`unknown_provider`。
//! 2. 发的那一句每次照资源目录读（`core/models/probe.txt`），去掉行尾的空白；读不了的 `internal_error`，记一行 `WARN`。
//! 3. key 先取好再试：配好的照这一刻的配置抄一份（和 `model.list` 同一个办法），候选的只取它自己那一个。`{value}` 只在
//!    这一次的内存里，不记、不存：最终值里写成一个不合密钥名字写法的引用（`miyu_models::onboard::value_key`），只认它。
//! 4. 试由会话那一层做（`miyu_session::probe`）：配好的列到的模型存进供应商的列表，候选的不存。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::merge::Resolved;
use miyu_config::secret::{Reference, Secret};
use miyu_models::onboard::{Candidate, value_key};
use miyu_models::provider::configured;
use miyu_session::{Probe, Probed, probe};

use crate::Core;
use crate::models::snapshot;
use crate::refusal::Refusal;

/// `provider.test` 的参数。
#[derive(Debug, Deserialize)]
struct TestParams {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    candidate: Option<CandidateParams>,
    #[serde(default)]
    model: Option<String>,
}

/// 还没写进配置的一家：每一格都可以不写，不认识的格不收。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateParams {
    #[serde(default)]
    driver: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    key: Option<KeyParam>,
    #[serde(default)]
    catalog: Option<String>,
}

/// key 的三种写法：`{"secret": …}`、`{"env": …}`、`{"value": …}`。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum KeyParam {
    Secret(String),
    Env(String),
    Value(String),
}

/// `provider.test`。
pub(crate) async fn test(core: &Core, params: Value) -> Result<Value, Refusal> {
    let params: TestParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    if params.model.as_deref() == Some("") {
        return Err(Refusal::BAD_PARAMS);
    }
    let data = Arc::clone(&core.model_data);
    data.wait().await;
    let text = core.resources.probe().map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", error = %error, "probe text unreadable");
        Refusal::INTERNAL
    })?;
    let text = text.trim_end();
    let model = params.model.as_deref();
    let probed = match (params.provider, params.candidate) {
        (Some(id), None) => {
            let snapshot = snapshot(core);
            let values = snapshot.resolved.values();
            if !configured(&values).contains(&id) {
                return Err(Refusal::UNKNOWN_PROVIDER);
            }
            let secret = |reference: &Reference| snapshot.secret(reference);
            let tried = Probe {
                values: &values,
                resolved: &snapshot.resolved,
                secret: &secret,
                id: &id,
                model,
                text,
                save: true,
            };
            probe(&data, tried).await
        }
        (None, Some(written)) => {
            let (candidate, key) = candidate(core, written);
            let id = candidate.id();
            let values = candidate.values();
            let reference = candidate.key.clone();
            let secret = |asked: &Reference| {
                (reference.as_ref() == Some(asked))
                    .then(|| key.clone())
                    .flatten()
            };
            let tried = Probe {
                values: &values,
                resolved: &Resolved::default(),
                secret: &secret,
                id: &id,
                model,
                text,
                save: false,
            };
            probe(&data, tried).await
        }
        _ => return Err(Refusal::BAD_PARAMS),
    };
    Ok(answer(probed))
}

/// 候选写成 [`Candidate`]，它的 key 先取好：`{value}` 的就是写的（照 `secret.set` 的规矩去掉前后空白，不收的当取不到），
/// 别的照这一刻的密钥文件、核心的环境。
fn candidate(core: &Core, written: CandidateParams) -> (Candidate, Option<Secret>) {
    let (key, value) = match written.key {
        None => (None, None),
        Some(KeyParam::Value(value)) => (Some(value_key()), Secret::new(&value).ok()),
        Some(KeyParam::Secret(name)) => {
            let reference = Reference::Secret(name);
            let value = core.config().secret(&reference);
            (Some(reference), value)
        }
        Some(KeyParam::Env(name)) => {
            let reference = Reference::Env(name);
            let value = core.config().secret(&reference);
            (Some(reference), value)
        }
    };
    let candidate = Candidate {
        driver: written.driver,
        base_url: written.base_url,
        key,
        catalog: written.catalog,
    };
    (candidate, value)
}

/// 写成回应。
fn answer(probed: Probed) -> Value {
    match probed {
        Probed::Worked {
            models,
            from_catalog,
            model,
            first_token_ms,
        } => json!({
            "ok": true,
            "models": models,
            "listed": if from_catalog { "catalog" } else { "provider" },
            "model": model,
            "first_token_ms": first_token_ms,
        }),
        Probed::Failed { stage, error } => {
            let mut said = json!({"class": error.class.as_str(), "message": error.message});
            if let Some(status) = error.status {
                said["status"] = json!(status);
            }
            json!({"ok": false, "stage": stage.as_str(), "error": said})
        }
    }
}
