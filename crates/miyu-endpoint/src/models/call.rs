//! 协议上的 `model.call`（`docs/blueprint/models.md`「协议」`model.call`、「怎么走」第十二条，施工 8-20）：经模型调用口的
//! 一次性入口叫一次模型或池，交回整段回答和用量。不进任何会话的日志，不推送。
//!
//! 1. 先查参数的样子：`purpose` 1 到 32 个小写字母、数字、`-`；`messages` 里 `system` 最多一条、只能在最前，后面至少一条、
//!    最后一条是 `user`；`text` 必写，`system`、`assistant` 的不能是空的，`user` 的字、图至少一样；`images` 只有 `user`
//!    能写；`max_tokens` 是 1 到 4294967295；`model` 不是空字。不对的 `bad_params`。
//! 2. 图照这个账号的 blob 认（`attach.rs` 的 `images`）：没有的 `unknown_attachment`，不是图的 `bad_params`。一条消息里先字
//!    后图。
//! 3. 照核心这一刻的配置（不算项目配置的最终值）冻结一份，交给一次性入口（`Models::one_shot()`；照剧本回的端口没有，答
//!    `no_model`）。没答成的照它的四种写成拒绝。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Message;
use miyu_models::provider::NOT_CONFIGURED;
use miyu_session::{Answer, Ask, ConfigSource, Turn, Unanswered};
use miyu_store::blob::Blobs;

use crate::Core;
use crate::attach;
use crate::refusal::Refusal;

/// `purpose` 最多几个字符。
const PURPOSE_MAX: usize = 32;

/// `model.call` 的参数：不认识的格不理，「可以不写」的写 `null` 等于没写。
#[derive(Debug, Deserialize)]
pub(crate) struct CallParams {
    #[serde(default)]
    model: Option<String>,
    purpose: String,
    messages: Vec<Said>,
    #[serde(default)]
    max_tokens: Option<u64>,
}

/// 一条消息。
#[derive(Debug, Deserialize)]
struct Said {
    role: Role,
    text: String,
    #[serde(default)]
    images: Option<Vec<String>>,
}

/// 谁说的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    System,
    User,
    Assistant,
}

/// 查过样子的一条消息：谁说的、字、图的哈希。
struct Line {
    role: Role,
    text: String,
    images: Vec<ContentHash>,
}

/// `model.call`。
pub(crate) async fn call(core: &Core, params: CallParams) -> Result<Value, Refusal> {
    if !purpose_ok(&params.purpose) || params.model.as_deref() == Some("") {
        return Err(Refusal::BAD_PARAMS);
    }
    let max_tokens = match params.max_tokens {
        None => None,
        Some(0) => return Err(Refusal::BAD_PARAMS),
        Some(limit) => Some(u32::try_from(limit).map_err(|_| Refusal::BAD_PARAMS)?),
    };
    let (system, lines) = shape(params.messages)?;
    let mut messages = Vec::with_capacity(lines.len());
    for line in lines {
        let mut blocks = Vec::new();
        if !line.text.is_empty() {
            blocks.push(Block::Text(Text { text: line.text }));
        }
        blocks.extend(attach::images(core, line.images).await?);
        // system 已经单拿出来了，后面只有 user、assistant。
        messages.push(match line.role {
            Role::Assistant => Message::Assistant { blocks },
            Role::System | Role::User => Message::User { blocks },
        });
    }
    let Some(one_shot) = core.models.one_shot() else {
        return Err(Refusal::no_model(NOT_CONFIGURED.to_string()));
    };
    let config = core.config_now().borrow().clone();
    let resolved = config.resolved().clone();
    let source: Arc<dyn ConfigSource> = config;
    let turn = Arc::new(Turn::new(resolved, source));
    let blobs = Blobs::new(core.root.blobs(&core.admin));
    let ask = Ask {
        model: params.model,
        purpose: params.purpose,
        system,
        messages,
        max_tokens,
    };
    match one_shot.call(&turn, &blobs, ask).await {
        Ok(answer) => Ok(answered(answer)),
        Err(Unanswered::UnknownModel(why)) => {
            tracing::debug!(target: "miyu::endpoint", why = %why, "unknown model");
            Err(Refusal::UNKNOWN_MODEL)
        }
        Err(Unanswered::NoModel(message)) => Err(Refusal::no_model(message)),
        Err(Unanswered::Cooling { message, wait_ms }) => Err(Refusal::cooling(message, wait_ms)),
        Err(Unanswered::Failed(error)) => Err(Refusal::model_failed(&error)),
    }
}

/// 用途合不合写法：1 到 32 个字符，只有小写字母、数字、`-`。
fn purpose_ok(purpose: &str) -> bool {
    (1..=PURPOSE_MAX).contains(&purpose.len())
        && purpose
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// 查消息的样子（见模块的说明），交回 system（没有的是空的）和后面几条。
fn shape(said: Vec<Said>) -> Result<(String, Vec<Line>), Refusal> {
    let mut lines = Vec::with_capacity(said.len());
    for each in said {
        let images = each
            .images
            .map(|images| {
                images
                    .iter()
                    .map(|text| ContentHash::parse(text).map_err(|_| Refusal::BAD_PARAMS))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let fine = match each.role {
            Role::User => {
                !each.text.is_empty() || images.as_ref().is_some_and(|all| !all.is_empty())
            }
            Role::System | Role::Assistant => !each.text.is_empty() && images.is_none(),
        };
        if !fine {
            return Err(Refusal::BAD_PARAMS);
        }
        lines.push(Line {
            role: each.role,
            text: each.text,
            images: images.unwrap_or_default(),
        });
    }
    let system = match lines.first() {
        Some(first) if first.role == Role::System => lines.remove(0).text,
        _ => String::new(),
    };
    let well = lines.last().is_some_and(|last| last.role == Role::User)
        && lines.iter().all(|line| line.role != Role::System);
    match well {
        true => Ok((system, lines)),
        false => Err(Refusal::BAD_PARAMS),
    }
}

/// 写成回应。
fn answered(answer: Answer) -> Value {
    json!({
        "text": answer.text,
        "provider": answer.provider,
        "model": answer.model,
        "usage": answer.usage,
    })
}
