//! 供应商的档案（`docs/blueprint/models.md`「在哪」`resources/models/profiles.toml`、「怎么走」第一条第 2、3 条，施工 8-6）：
//! 认得出的供应商可以少写，驱动、地址、开关照档案推。档案是资源目录里的 TOML，核心读成 JSON 再交进来（这一层不读 TOML
//! 的资源文件，`models.md`「在哪」末尾）。
//!
//! 档案只有用得上的几格：驱动、地址、`openai-chat` 的开关、一张图怎么算（8-6），`[npm]`：目录里的 AI SDK 包名 → 驱动
//! （8-7，照目录推驱动）。另配的头、占位工具、找 key 的环境变量、本机服务随 8-11、8-14。8-6 加的「能收哪些输入」8-7 拿掉了：
//! 照模型资料（目录、手写的，「施工时定的」8-7）。

use std::collections::BTreeMap;

use serde::Deserialize;

use miyu_drivers::openai_chat::{
    Compat, Continuation, ContinuationField, OutputLimit, ReasoningField, ReasoningReplay,
};

/// 读好的档案。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profiles {
    /// 目录里的 AI SDK 包名 → 驱动（`openai-chat` 这类，施工 8-7）：照目录推驱动时查（`models.md`「怎么走」第一条第 2 条）。
    #[serde(default)]
    pub npm: BTreeMap<String, String>,
    /// 认得出的供应商：编号（照目录里的编号）到它的档案。
    #[serde(default)]
    pub providers: BTreeMap<String, Profile>,
}

/// 一家认得出的供应商的档案：每一格都可以不写。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// 驱动，写法同配置的 `driver`。
    #[serde(default)]
    pub driver: Option<String>,
    /// 地址。
    #[serde(default)]
    pub base_url: Option<String>,
    /// `openai-chat` 的开关。
    #[serde(default)]
    pub compat: Option<CompatSpec>,
    /// 一张图怎么算 token：现在只有 `deepseek`（官方计算器的算法）。不写照策略的固定数。
    #[serde(default)]
    pub image_tokens: Option<ImageTokens>,
}

/// 一张图怎么算 token 的算法。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageTokens {
    /// DeepSeek 官方计算器的算法（`miyu_drivers::DeepSeekImages`）。
    DeepSeek,
}

/// 档案里 `compat` 的写法（`models.md`「对外的样子」`compat` 那张表）：一格对 [`Compat`] 的一格，没写的用驱动的默认。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatSpec {
    /// `"max_tokens"`、`"max_completion_tokens"`。
    #[serde(default)]
    pub output_limit: Option<OutputLimitSpec>,
    /// `"drop"`，或者 `{ replay, always }`。
    #[serde(default)]
    pub reasoning: Option<ReasoningSpec>,
    /// 要不要在流里报用量。
    #[serde(default)]
    pub stream_usage: Option<bool>,
    /// `"none"`，或者 `{ field, path }`。
    #[serde(default)]
    pub continuation: Option<ContinuationSpec>,
}

/// 输出上限写在哪个字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputLimitSpec {
    /// `max_tokens`。
    MaxTokens,
    /// `max_completion_tokens`。
    MaxCompletionTokens,
}

/// 思考怎么回传。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum ReasoningSpec {
    /// `"drop"`：不回传。
    Word(DropWord),
    /// `{ replay = "reasoning_content" 或 "reasoning", always = true 或 false }`。
    Replay {
        /// 写进哪个字段。
        replay: ReasoningFieldSpec,
        /// 没有思考时也写。
        always: bool,
    },
}

/// 只认 `"drop"` 这一个词。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DropWord {
    /// 不回传。
    Drop,
}

/// 思考写进哪个字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningFieldSpec {
    /// `reasoning_content`。
    ReasoningContent,
    /// `reasoning`。
    Reasoning,
}

/// 会不会接着写被打断的回复。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum ContinuationSpec {
    /// `"none"`：不会。
    Word(NoneWord),
    /// `{ field = "prefix" 或 "partial", path = "…" }`。
    Prefix {
        /// 半截那条 assistant 上加哪个字段。
        field: ContinuationFieldSpec,
        /// 发到地址后面的哪一截。
        path: String,
    },
}

/// 只认 `"none"` 这一个词。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoneWord {
    /// 不会接着写。
    None,
}

/// 接着写的时候加的字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContinuationFieldSpec {
    /// `prefix`。
    Prefix,
    /// `partial`。
    Partial,
}

impl Profiles {
    /// 读档案：核心把 TOML 读成的 JSON。
    ///
    /// # Errors
    ///
    /// 不是这个形状（多了不认识的格、写法不对）：原因写明是档案。
    pub fn parse(json: &serde_json::Value) -> Result<Profiles, String> {
        Profiles::deserialize(json)
            .map_err(|error| format!("models/profiles.toml not readable: {error}"))
    }
}

impl CompatSpec {
    /// 一格格盖在驱动的默认上面。
    pub fn compat(&self) -> Compat {
        let mut compat = Compat::default();
        if let Some(limit) = self.output_limit {
            compat.output_limit = match limit {
                OutputLimitSpec::MaxTokens => OutputLimit::MaxTokens,
                OutputLimitSpec::MaxCompletionTokens => OutputLimit::MaxCompletionTokens,
            };
        }
        if let Some(reasoning) = &self.reasoning {
            compat.reasoning = match reasoning {
                ReasoningSpec::Word(DropWord::Drop) => ReasoningReplay::Drop,
                ReasoningSpec::Replay { replay, always } => ReasoningReplay::Replay {
                    field: match replay {
                        ReasoningFieldSpec::ReasoningContent => ReasoningField::ReasoningContent,
                        ReasoningFieldSpec::Reasoning => ReasoningField::Reasoning,
                    },
                    always: *always,
                },
            };
        }
        if let Some(usage) = self.stream_usage {
            compat.stream_usage = usage;
        }
        if let Some(continuation) = &self.continuation {
            compat.continuation = match continuation {
                ContinuationSpec::Word(NoneWord::None) => Continuation::None,
                ContinuationSpec::Prefix { field, path } => Continuation::Prefix {
                    field: match field {
                        ContinuationFieldSpec::Prefix => ContinuationField::Prefix,
                        ContinuationFieldSpec::Partial => ContinuationField::Partial,
                    },
                    path: path.clone(),
                },
            };
        }
        compat
    }
}

#[cfg(test)]
mod tests;
