//! 模型资料（施工 6-3 上，`docs/blueprint/core.md`「模型」）：资源目录里的 `models/models-dev.json`，从 models.dev 的
//! `api.json` 抽出来的，只留驱动认得的供应商，每个模型只留窗口（`limit.context`）和最大输出（`limit.output`），形状照
//! models.dev 原来的。照（供应商，模型名）查。

use std::collections::BTreeMap;

use serde::Deserialize;

/// 一个模型的资料：没写的是没有。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModelFacts {
    /// 上下文窗口。
    pub window: Option<u64>,
    /// 最大输出。
    pub max_output: Option<u64>,
}

/// 读进来的模型资料。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelTable(BTreeMap<String, BTreeMap<String, ModelFacts>>);

/// 文件的样子。
#[derive(Deserialize)]
struct File {
    /// 从哪抽的，给人看；读的时候只查它在。
    #[serde(rename = "source")]
    _source: String,
    /// 哪天抽的，同上。
    #[serde(rename = "fetched")]
    _fetched: String,
    providers: BTreeMap<String, Provider>,
}

#[derive(Deserialize)]
struct Provider {
    models: BTreeMap<String, Model>,
}

#[derive(Deserialize)]
struct Model {
    limit: Limit,
}

#[derive(Deserialize)]
struct Limit {
    context: Option<u64>,
    output: Option<u64>,
}

impl ModelTable {
    /// 读原文。
    ///
    /// # Errors
    ///
    /// 不是这个形状的 JSON：原因写明是模型资料。
    pub fn parse(text: &str) -> Result<ModelTable, String> {
        let file: File = serde_json::from_str(text)
            .map_err(|error| format!("models/models-dev.json not readable: {error}"))?;
        Ok(ModelTable(
            file.providers
                .into_iter()
                .map(|(provider, entry)| {
                    let models = entry
                        .models
                        .into_iter()
                        .map(|(name, model)| {
                            let facts = ModelFacts {
                                window: model.limit.context,
                                max_output: model.limit.output,
                            };
                            (name, facts)
                        })
                        .collect();
                    (provider, models)
                })
                .collect(),
        ))
    }

    /// 这家供应商的这个模型的资料；查不到的都是没有。
    pub fn find(&self, provider: &str, model: &str) -> ModelFacts {
        self.0
            .get(provider)
            .and_then(|models| models.get(model))
            .copied()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
