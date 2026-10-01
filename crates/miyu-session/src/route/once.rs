//! 模型调用口的一次性入口（`docs/blueprint/models.md`「怎么走」第十二条第 4 到 7 条，施工 8-20）：发一次、拿整段回答和
//! 用量，不属于哪个会话。和会话的路由是同一个 [`Routes`]（核心经 `Models::one_shot()` 拿到），挑、发、记冷却都调底子，
//! 冷却表、池的指针两个入口共用。
//!
//! 1. 先等目录读完。写了引用的照这一刻不算项目配置的最终值认（`record`，解析不出是 `unknown_model`），再解析；没写的照
//!    `models.chat`（没配、解析不出是 `no_model`）。
//! 2. 照底子挑一个候选：key 照用途钉，没有换过去的 key、没有说到一半断了的；钉住的池第一次照指针取成员，这一次里出错
//!    从刚才那个往下绕。
//! 3. 交进去的图，挑中的模型不收图的：不发，交 `model_failed`（`other`），不记冷却、不换。
//! 4. 发：没有人叫停，增量拼成正文（`once/reply.rs`），不往外推。成了交回正文、真发给的供应商和模型、用量。
//! 5. 出错了、底子说换了端点、别的候选这时就能用的：当场换下一个，最多换 [`FAILOVERS`] 次。只剩等的、只有一个候选的、
//!    不换的分类：不等，交 `model_failed`。收到过增量才出错的也换（交给底子的「说到一半断了」是假的）：半截没人看到。
//! 6. 记一行 `INFO model call`（成了）或 `INFO model call failed`（没成），目标 `miyu::session`，不带会话编号。

mod reply;

use std::collections::BTreeMap;

use miyu_kernel::block::Block;
use miyu_kernel::event::{CallError, ErrorClass, Usage};
use miyu_kernel::request::{Message, Request};
use miyu_models::pools::Member;
use miyu_models::provider::{self, NOT_CONFIGURED, NoModel};
use miyu_models::reference::record;
use miyu_store::blob::Blobs;

use super::Routes;
use super::base::Seat;
use super::choice::{Choice, Unsent};
use super::exchange::{Exchanged, exchange};
use super::lists::listing_texts;
use crate::TARGET;
use crate::config::TurnConfig;
use reply::Reply;

/// 一次调用里出错换端点最多换几次：和会话一轮里再来的次数一样（`kernel/session.md`「出错再来」）。
const FAILOVERS: usize = 5;

/// 一次性入口（施工 8-20）：核心一份，可以复制。
#[derive(Clone)]
pub struct OneShot {
    routes: Routes,
}

/// 交进去的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// 模型 `<供应商>/<模型>` 或 `@池`；没有的照这一刻的 `models.chat`。
    pub model: Option<String>,
    /// 用途：一个短名字（例如 `vision`、`platform`），记运行日志，key 照它钉。
    pub purpose: String,
    /// system；空的不发。
    pub system: String,
    /// 几条消息，照先后：`user`、`assistant`，`user` 可以带图。不带工具。
    pub messages: Vec<Message>,
    /// 最多输出多少 token；没有的照供应商的默认。
    pub max_tokens: Option<u32>,
}

/// 交回的：整段回答。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// 正文：正文块的字照先后接起来，思考不要；没有正文的是空字。
    pub text: String,
    /// 真发给的供应商编号（换过端点的是最后成了的那一个）。
    pub provider: String,
    /// 真发给的模型名。
    pub model: String,
    /// 用量；供应商没报的没有。
    pub usage: Option<Usage>,
}

/// 没答成（「怎么走」第十二条第 5 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unanswered {
    /// 写了的引用解析不出：原话。
    UnknownModel(String),
    /// 没有能用的模型：原话。
    NoModel(String),
    /// 候选不止一个、全在冷却，没发。
    Cooling {
        /// 原话：每个候选为什么、到什么时候。
        message: String,
        /// 最早恢复的那一个还要等多少毫秒。
        wait_ms: u64,
    },
    /// 发了、出错了（换不了、换够了），或者模型不收图、增量对不上、编码要的 blob 取不出来：和 `model.called` 的一样。
    Failed(CallError),
}

impl Unanswered {
    /// 协议上的原因码：`unknown_model`、`no_model`、`cooling`、`model_failed`。
    pub fn reason(&self) -> &'static str {
        match self {
            Unanswered::UnknownModel(_) => "unknown_model",
            Unanswered::NoModel(_) => "no_model",
            Unanswered::Cooling { .. } => "cooling",
            Unanswered::Failed(_) => "model_failed",
        }
    }
}

impl From<NoModel> for Unanswered {
    fn from(NoModel(message): NoModel) -> Unanswered {
        Unanswered::NoModel(message)
    }
}

impl From<Unsent> for Unanswered {
    fn from(unsent: Unsent) -> Unanswered {
        match unsent {
            Unsent::NoModel(no_model) => no_model.into(),
            Unsent::Cooling { message, wait_ms } => Unanswered::Cooling { message, wait_ms },
        }
    }
}

impl OneShot {
    /// 在 `routes` 上的一次性入口：冷却表、池的指针和它造的会话的路由共用。
    pub fn new(routes: Routes) -> OneShot {
        OneShot { routes }
    }

    /// 发一次：照这一次的配置 `config`（一次性的照这一刻不算项目配置的最终值），图从 `blobs`（调的一方那个账号的）取。
    /// 记一行运行日志。
    ///
    /// # Errors
    ///
    /// 没答成的四种（[`Unanswered`]）。
    pub async fn call(
        &self,
        config: &TurnConfig,
        blobs: &Blobs,
        ask: Ask,
    ) -> Result<Answer, Unanswered> {
        let purpose = ask.purpose.clone();
        let answered = self.answer(config, blobs, ask).await;
        match &answered {
            Ok(answer) => {
                let input = answer
                    .usage
                    .map(|usage| usage.uncached + usage.cache_read + usage.cache_write);
                tracing::info!(
                    target: TARGET,
                    purpose = purpose.as_str(),
                    provider = answer.provider.as_str(),
                    model = answer.model.as_str(),
                    input,
                    output = answer.usage.map(|usage| usage.output),
                    "model call"
                );
            }
            Err(unanswered) => {
                let class = match unanswered {
                    Unanswered::Failed(error) => Some(error.class.as_str()),
                    _ => None,
                };
                tracing::info!(
                    target: TARGET,
                    purpose = purpose.as_str(),
                    reason = unanswered.reason(),
                    class,
                    "model call failed"
                );
            }
        }
        answered
    }

    /// 见模块的说明，不记日志。
    async fn answer(
        &self,
        config: &TurnConfig,
        blobs: &Blobs,
        ask: Ask,
    ) -> Result<Answer, Unanswered> {
        let routes = &self.routes;
        routes.data.wait().await;
        let values = config.resolved.values();
        let text = match ask.model {
            Some(text) => {
                record(&values, &text).map_err(|NoModel(why)| Unanswered::UnknownModel(why))?;
                text
            }
            None => provider::chat(&values).ok_or_else(|| NoModel(NOT_CONFIGURED.to_string()))?,
        };
        let resolved = routes.resolve(&values, &text)?;
        let images = ask.messages.iter().any(has_image);
        let request = Request {
            tools: Vec::new(),
            system: ask.system,
            messages: ask.messages,
            stable: 0,
            continuation: false,
        };
        let texts = listing_texts().map_err(|why| Unanswered::Failed(other(why)))?;
        let moved = BTreeMap::new();
        let mut held: Option<Member> = None;
        let mut switched = 0;
        loop {
            let seat = Seat {
                seed: &ask.purpose,
                moved: &moved,
                held: held.as_ref(),
                sticky: None,
            };
            let (picked, _) = routes.pick(config, &values, &resolved, &seat)?;
            held.clone_from(&picked.choice.member);
            let ready =
                routes.ready(config, &picked.choice, None, texts.clone(), ask.max_tokens)?;
            if images && !ready.call.inputs.images {
                return Err(Unanswered::Failed(no_images(&picked.choice)));
            }
            let mut reply = Reply::default();
            let exchanged = exchange(
                &ready,
                &request,
                blobs,
                std::future::pending(),
                |progress| reply.take(progress),
            )
            .await;
            let classified = match exchanged {
                Exchanged::Missing(error) => {
                    picked.failed(&error.class, None, false);
                    return Err(Unanswered::Failed(error));
                }
                Exchanged::Cancelled => unreachable!("没有人叫停：std::future::pending 不会完成"),
                Exchanged::Ended { usage, error: None } => {
                    picked.succeeded();
                    return reply.answer(&picked.choice.target, usage);
                }
                Exchanged::Ended {
                    error: Some(classified),
                    ..
                } => classified,
            };
            let switch = picked.failed(&classified.error.class, classified.retry_after_ms, false);
            if !switch.failover || switch.wait_ms.is_some() || switched == FAILOVERS {
                return Err(Unanswered::Failed(classified.error));
            }
            switched += 1;
        }
    }
}

/// 这条消息带图。
fn has_image(message: &Message) -> bool {
    match message {
        Message::User { blocks } => blocks.iter().any(|block| matches!(block, Block::Image(_))),
        Message::Assistant { .. } | Message::Tool { .. } => false,
    }
}

/// 挑中的模型不收图：不发。
fn no_images(choice: &Choice) -> CallError {
    let name = format!("{}/{}", choice.target.provider.id, choice.target.model);
    other(format!("model {name:?} does not take images"))
}

/// 分类「其他」、没有状态码的一句。
fn other(message: String) -> CallError {
    CallError {
        class: ErrorClass::Unclassified,
        message,
        status: None,
    }
}
