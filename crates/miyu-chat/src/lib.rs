//! 群聊内核（`docs/blueprint/chat.md`，`docs/designs/18-通讯平台.md` 第一节）：通讯平台里和平台无关的那一层，第 2 层的
//! 纯逻辑。进来的是字和事件，出去的是判定，不碰磁盘、网络、时钟；软件包 `miyu-onebot` 链接它，以后别的平台的桥也链接
//! 同一个库，它不编进核心（`docs/designs/01-架构.md` 第九节）。
//!
//! 现在有七块：
//!
//! - 场所规则（`chat.md` 第一条，施工 O-1）：[`Rules::parse`] 读一组规则文件（[`File`]，出厂的和系统的），照文件名排好
//!   先后，写错的变成 [`Problem`]，其余照收（[`Parsed`]）；[`Rules::resolve`] 套到一个场所（[`Venue`]）上，得出每一项的值
//!   和来处（[`Resolved`]、[`Entry`]、[`Origin`]）。读文件、文件大小、是不是 UTF-8 由读文件的一方管（`chat.md` 施工时定的
//!   第 5 条）。场所只能由 [`Venue::new`] 造，造的时候拼好编号；编号的拼和解也在这里（`chat.md` 第七条第 1 条，施工
//!   O-12）：场所 [`Venue::id`]、[`Venue::parse`]，平台上的人 [`person`]、[`parse_person`]。
//! - 进站链与限流（`chat.md` 第二条，施工 O-5）：[`Chain::judge`] 让一条进来的消息（[`Inbound`]，和主动回复判断共用的
//!   那几格在 [`Said`]）照顺序过自带的五条规则，给出 [`Verdict`]；[`rate_full`] 说额度满了没有；[`gate()`] 回答核心自己
//!   开的回合现在开还是推迟。限流、睡眠只能从场所规则的原文读（[`Rate::read`]、[`Sleep::read`]）。桥要的两样也在这里
//!   （施工 O-12 下）：[`addressed()`] 算一条消息是不是冲她来的，填 [`Said::addressed`]；[`Base64::reveal`] 交出正文里
//!   的 base64 解出来的字，违规关键词查它，判官也看它（[`Ask::decoded`]）。
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
//!   （[`JudgeMessage`]），说明的原文由外面交进来（[`JudgeTexts`]）；[`read()`] 从判官的回答里读出 [`Judgement`]，读不出来的是
//!   [`Unreadable`]。
//!
//! - 出厂参数和按场所改（`chat.md` 第八条，施工 O-15）：[`Params::read`] 照声明读出厂文件，有一条问题就整份不用；
//!   [`Params::at`] 套上场所规则改的几项（场所规则里写同名的表，展开成一项一项）。第二到第六条的参数（[`Base64`]、
//!   [`Chatty`]、[`Outbound`]）只能从它拿，判官的几项在 [`Judge`]。
//!
//! 出站队列随后面的 O 步加。

mod chatty;
mod inbound;
mod judge;
mod outbound;
mod params;
mod rules;

pub use chatty::{
    Bonus, BonusCtx, Chatty, Conditions, Dispatch, Facts, Hit, Judgement, Kind, Lift, LiftCtx,
    Line, Lines, Pending, Reply, Restraint, Route, Score, Status, Supersede, Window, conditions,
    dispatch, pressure, route, score, supersede,
};

pub use inbound::{
    Base64, Chain, Clock, Ctx, Flag, Gate, Inbound, InboundRule, Moderation, Outcome, Rate, Said,
    Sleep, Standing, Step, Verdict, Why, addressed, gate, rate_full,
};
pub use judge::{
    Ask, Judge, JudgeMessage, JudgeRole, JudgeSources, JudgeTexts, Mode, Unreadable, read, request,
};
pub use outbound::{
    Out, OutChain, OutCtx, OutStep, OutWhy, Outbound, OutboundRule, Outgoing, Sent, Since, Target,
    plain, split,
};
pub use params::Params;
pub use rules::{
    Entry, File, Origin, Parsed, Problem, Resolved, Rules, Source, Venue, VenueKind, parse_person,
    person,
};
