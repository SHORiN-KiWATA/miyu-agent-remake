//! `session.configure` 的参数（施工 8-10，`docs/blueprint/models.md`「协议」）：哪个会话、换成的模型或 `@池`（`model`）、
//! 会话给一个模型记的思考强度（`effort`，施工 8-18）。
//!
//! 1. 先查参数：`model`、`effort` 至少写一个；`model` 不是字、是空字，`effort` 不是对象、它的 `model` 不是不空的字、`level`
//!    没写或者不是不空的字也不是 `null`：`bad_params`，不找会话。
//! 2. 找会话以后再照这时的配置查（[`effort()`]）：`effort.model` 要是模型、那一家配了（不然 `unknown_model`），`level` 要在那个
//!    模型这时的档位里（不然 `unknown_effort`，原话记一行 `DEBUG unknown effort`）；`null` 不查。

use serde::Deserialize;
use serde_json::Value;

use miyu_kernel::event::Effort;
use miyu_models::effort;
use miyu_models::provider::NoModel;

use crate::Core;
use crate::refusal::Refusal;

/// `session.configure` 的参数：`session` 必写，`model`、`effort` 可以不写（写 `null` 等于没写），不是这个形状的读不成
/// （`bad_params`）。
#[derive(Debug, Deserialize)]
pub(crate) struct ConfigureParams {
    /// 哪个会话。
    pub(crate) session: String,
    /// 换成的引用，还没解析。
    #[serde(default)]
    model: Option<String>,
    /// 换的思考强度，还没查：形状在 [`ConfigureParams::asked`] 里查。
    #[serde(default)]
    effort: Option<Value>,
}

/// 写了的思考强度，形状查过、还没照配置查。
#[derive(Debug)]
pub(crate) struct Asked {
    /// 哪个模型，原样。
    model: String,
    /// 哪一档；`None` 是清掉。
    level: Option<String>,
}

impl ConfigureParams {
    /// 换成的引用（原样）、换的思考强度（「施工时定的」8-10、8-18）。
    ///
    /// # Errors
    ///
    /// 两样都没写；`model` 是空字；`effort` 不是那个形状：`bad_params`。
    pub(crate) fn asked(&self) -> Result<(Option<&str>, Option<Asked>), Refusal> {
        let model = match self.model.as_deref() {
            Some("") => return Err(Refusal::BAD_PARAMS),
            model => model,
        };
        let effort = self.effort.as_ref().map(shape).transpose()?;
        match (model, effort) {
            (None, None) => Err(Refusal::BAD_PARAMS),
            (model, effort) => Ok((model, effort)),
        }
    }
}

/// `effort` 的形状：`{"model": 不空的字, "level": 不空的字或 null}`，`level` 要写。
fn shape(value: &Value) -> Result<Asked, Refusal> {
    let Value::Object(fields) = value else {
        return Err(Refusal::BAD_PARAMS);
    };
    let model = match fields.get("model") {
        Some(Value::String(model)) if !model.is_empty() => model.clone(),
        _ => return Err(Refusal::BAD_PARAMS),
    };
    let level = match fields.get("level") {
        Some(Value::Null) => None,
        Some(Value::String(level)) if !level.is_empty() => Some(level.clone()),
        _ => return Err(Refusal::BAD_PARAMS),
    };
    Ok(Asked { model, level })
}

/// 照这时不算项目配置的最终值查一格思考强度（施工 8-18）：先等目录读完；`model` 要是模型、那一家配了；`level` 规整以后
/// （`none`、`disabled` 读成 `off`）要在那个模型这时的档位里，`null` 不查。交回内核记的那一格。
///
/// # Errors
///
/// 不是模型、那一家没配：`unknown_model`；不在档位里：`unknown_effort`（原话各记一行 `DEBUG`）。
pub(crate) async fn effort(core: &Core, asked: Asked) -> Result<Effort, Refusal> {
    core.model_data.wait().await;
    let resolved = core.config().resolved().clone();
    let levels = core
        .model_data
        .with(|knowledge| effort::levels_for(&resolved, knowledge, &asked.model));
    let (model, levels) = levels.map_err(|NoModel(why)| {
        tracing::debug!(target: "miyu::endpoint", why = %why, "unknown model");
        Refusal::UNKNOWN_MODEL
    })?;
    let level = asked
        .level
        .map(|level| effort::normalize(&level).to_string());
    if let Some(level) = &level
        && !levels.contains(level)
    {
        tracing::debug!(target: "miyu::endpoint", model = model.as_str(), level = level.as_str(), "unknown effort");
        return Err(Refusal::UNKNOWN_EFFORT);
    }
    Ok(Effort { model, level })
}
