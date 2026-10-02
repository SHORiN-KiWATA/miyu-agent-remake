//! 模型默认的思考强度不在档位里的（`docs/blueprint/config.md`「报错」的 `unknown_effort`，`models.md`「怎么走」第十一条第
//! 2 条，施工 8-18）：照 `bad_reference` 的办法，读进来以后另查、只报不丢，请求照没写发。
//!
//! 档位照核心一份的模型资料算（档案、目录、手写的 `reasoning`），和请求时同一个算法（`miyu_models::facts`）。目录读完以前
//! 不查：起来那一刻目录还没读，查了会把每一个都报成错。那一家用不了（推不出驱动、地址）的不查。

use miyu_config::Layer;
use miyu_config::merge::Resolved;
use miyu_config::parse::Parsed;
use miyu_config::problem::Problem;
use miyu_models::facts::facts;
use miyu_models::provider;
use miyu_session::ModelData;

/// 一层配置 `parsed`（当成 `layer` 读的）里写的思考强度，照最终值 `resolved` 和模型资料 `data` 查。
pub(super) fn unknown(
    data: &ModelData,
    parsed: &Parsed,
    layer: Layer,
    resolved: &Resolved,
) -> Vec<Problem> {
    if !data.is_loaded() {
        return Vec::new();
    }
    let values = resolved.values();
    data.with(|knowledge| {
        miyu_models::effort::unknown(parsed, layer, &|id, model| {
            let provider = provider::provider(&values, knowledge, id).ok()?;
            let (facts, _) = facts(resolved, knowledge, &provider, model);
            Some(facts.levels().to_vec())
        })
    })
}
