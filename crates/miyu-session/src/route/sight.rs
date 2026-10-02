//! 会话入口替看不了图的模型看图（施工 8-17，`docs/blueprint/models.md`「怎么走」第十三条第 4 条）：内核交来的转述请求经一次性
//! 入口（`route/once.rs`）发给这一轮的 `models.vision`，用途 `vision`，图从会话属主的 blob 取。在派出去的任务里走完，带着会话
//! 的 span：一次性入口那一行 `model call` 也带着会话编号。
//!
//! - 没配 `models.vision` 的不发，当场没成。
//! - 一次性入口没答成的（四种），没成，原话照它的。
//! - 答成了：正文去掉前后空白，是空的没成；真发给的供应商、模型写不成编号的（照理不会有）也没成。

use tracing::Instrument;

use miyu_kernel::id::{AccountId, ModelName, ProviderId};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_models::settings::UseSettings;
use miyu_store::blob::Blobs;

use super::Routes;
use super::once::{Answer, Ask, OneShot, Unanswered};
use crate::config::TurnConfig;
use crate::port::Sight;

/// 一次性入口的用途：记运行日志，key 照它钉。
const PURPOSE: &str = "vision";

/// 没配 `models.vision` 时的原话。
const NO_VISION: &str = "no vision model configured: set models.vision";

/// 回答是空的时的原话。
const EMPTY: &str = "the vision reply has no text";

/// 发一次转述：照这一轮的配置 `config`，结果交给 `sight`。马上返回。用量记在会话的属主 `owner` 的账上（施工 8-15）。
pub(super) fn spawn(
    (routes, owner): (Routes, AccountId),
    blobs: Blobs,
    request: Request,
    config: &TurnConfig,
    sight: Sight,
) {
    let values = config.resolved.values();
    let Some(vision) = UseSettings::from(&values).vision else {
        sight.unseen(NO_VISION.to_string());
        return;
    };
    let ask = Ask {
        model: Some(vision),
        purpose: PURPOSE.to_string(),
        system: request.system,
        messages: request.messages,
        max_tokens: None,
        owner,
    };
    let config = TurnConfig::clone(config);
    let task = async move {
        match OneShot::new(routes).call(&config, &blobs, ask).await {
            Ok(answer) => match seen(answer) {
                Ok((model, text)) => sight.seen(model, text),
                Err(why) => sight.unseen(why),
            },
            Err(unanswered) => sight.unseen(why(unanswered)),
        }
    };
    tokio::spawn(task.instrument(tracing::Span::current()));
}

/// 答成了的：替它看的模型和去掉前后空白的转述；空的、写不成编号的没成。
fn seen(answer: Answer) -> Result<(Model, String), String> {
    let text = answer.text.trim();
    if text.is_empty() {
        return Err(EMPTY.to_string());
    }
    let endpoint = ProviderId::parse(&answer.provider).map_err(|e| e.to_string())?;
    let model = ModelName::parse(&answer.model).map_err(|e| e.to_string())?;
    Ok((Model { endpoint, model }, text.to_string()))
}

/// 没答成的原话：模型出错的前面带分类。
fn why(unanswered: Unanswered) -> String {
    match unanswered {
        Unanswered::UnknownModel(message) | Unanswered::NoModel(message) => message,
        Unanswered::Cooling { message, .. } => message,
        Unanswered::Failed(error) => format!("{}: {}", error.class.as_str(), error.message),
    }
}
