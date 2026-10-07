//! 群聊内核（`docs/blueprint/chat.md`，`docs/designs/18-通讯平台.md` 第一节）：通讯平台里和平台无关的那一层，第 2 层的
//! 纯逻辑。进来的是字和事件，出去的是判定，不碰磁盘、网络、时钟；软件包 `miyu-onebot` 链接它，以后别的平台的桥也链接
//! 同一个库，它不编进核心（`docs/designs/01-架构.md` 第九节）。
//!
//! 现在有六块：
//!
//! - 场所规则（`chat.md` 第一条，施工 O-1）：[`Rules::parse`] 读一组规则文件（[`File`]，出厂的和系统的），照文件名排好
//!   先后，写错的变成 [`Problem`]，其余照收；[`Rules::resolve`] 套到一个场所（[`Venue`]）上，得出每一项的值和来处
//!   （[`Resolved`]、[`Origin`]）。读文件、文件大小、是不是 UTF-8 由读文件的一方管（`chat.md` 施工时定的第 5 条）。
//! - 进站链与限流（`chat.md` 第二条，施工 O-5）：[`Chain::judge`] 让一条进来的消息（[`Inbound`]）照顺序过自带的五条
//!   规则，给出 [`Verdict`]；[`rate_full`] 说额度满了没有；[`gate()`] 回答核心自己开的回合现在开还是推迟。限流、睡眠从
//!   场所规则的原文读（[`Rate::read`]、[`Sleep::read`]）。
//!
//! - 主动回复判断的上半（`chat.md` 第三条，施工 O-7）：[`conditions`] 算一条消息（[`Facts`]）成立了哪些触发条件，
//!   [`route`] 定走哪条路，[`score()`] 拿判官的回答（[`Judgement`]）算分、跟门槛比；冷静机制照近期发言量（[`pressure`]）
//!   抬门槛。两个插槽：加值项 [`Bonus`]、门槛修正 [`Lift`]。
//! - 主动回复判断的下半（`chat.md` 第四条，施工 O-9）：[`supersede`] 看一条消息顶替了同一个人前面还没回完的哪一条
//!   （[`Pending`]），接过去还是几条一起重判；[`dispatch`] 把承诺要回的一条分派到主线还是支线（[`Lines`]）。
//!
//! - 出站链与纯文本（`chat.md` 第五条，施工 O-10）：[`OutChain::judge`] 让她要说出去的一条（[`Outgoing`]）照顺序过自带的
//!   三条规则（清理、去重、引用和 @），给出 [`Out`]；插槽是 [`OutboundRule`]。纯文本的形态：[`plain()`] 把 Markdown 转成
//!   纯文本，[`split()`] 把太长的按段拆开。
//!
//! - 判官的请求和回答（`chat.md` 第六条，施工 O-11）：[`request()`] 照 `model.call` 的形状拼出一条 `system`、一条 `user`
//!   （[`Message`]），说明的原文由外面交进来（[`JudgeTexts`]）；[`read()`] 从判官的回答里读出 [`Judgement`]，读不出来的是
//!   [`Unreadable`]。
//!
//! 出站队列随后面的 O 步加。

mod chatty;
mod inbound;
mod judge;
mod outbound;
mod rules;

pub use chatty::{
    Bonus, BonusCtx, Chatty, Conditions, Dispatch, Facts, Hit, Judgement, Kind, Lift, LiftCtx,
    Line, Lines, Pending, Reply, Restraint, Route, Score, Status, Supersede, Window, conditions,
    dispatch, pressure, route, score, supersede,
};

pub use inbound::{
    Base64, Chain, Clock, Ctx, Flag, Gate, Inbound, InboundRule, Moderation, Outcome, Rate, Sleep,
    Standing, Step, Verdict, Why, gate, rate_full,
};
pub use judge::{Ask, JudgeSources, JudgeTexts, Message, Mode, Role, Unreadable, read, request};
pub use outbound::{
    Out, OutChain, OutCtx, OutStep, OutWhy, Outbound, OutboundRule, Outgoing, Sent, Since, Target,
    plain, split,
};
pub use rules::{Entry, File, Origin, Problem, Read, Resolved, Rules, Source, Venue, VenueKind};
