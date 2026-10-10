//! 主动回复判断的上半（`docs/blueprint/chat.md` 第三条，`docs/designs/18-通讯平台.md` 第七节、Q4、Q23，施工 O-7）：线路规程
//! 「看情况插话」`chatty` 的核心。一条消息成立了哪些触发条件（[`conditions`]），走哪条路（[`route`]），拿到判官的回答
//! 以后算分、跟门槛比（[`score()`]）；冷静机制抬门槛（[`pressure`]）。下半（`chat.md` 第四条，施工 O-9）在 `supersede`、
//! `dispatch` 两个文件：顶替窗口（[`supersede()`]）和主线、支线的分派（[`dispatch()`]）。
//!
//! 照插件的形状写两个插槽（18 第十四节）：加值项 [`Bonus`]、门槛修正 [`Lift`]。自带五个加值项、一个门槛修正，每个一个
//! 文件；现在没有往里加的入口，加的是扩展，随插件那一步。
//!
//! 纯逻辑：她最近回过谁（[`Reply`]）、此刻（[`Clock`]），由外面从场所会话的日志投影出来交进来；参数（[`Chatty`]）从
//! `chat.md` 第八条的 [`Params`](crate::Params) 拿（施工 O-15）。抽样照哈希，不碰随机源（`02-内核.md` K1）：同一份日志回放出同样的结果。

mod after_speaking;
mod continuation;
mod direct;
mod dispatch;
mod moderation;
mod probability;
mod restraint;
mod sample;
mod score;
mod supersede;

pub use dispatch::{Dispatch, Line, Lines, dispatch};
pub use restraint::pressure;
pub use score::{Judgement, Score, score};
pub use supersede::{Pending, Status, Supersede, supersede};

use miyu_kernel::id::{ExternalId, Seq, VenueId};
use miyu_kernel::time::Timestamp;

use crate::{Clock, Flag, Said, Standing};

/// 一条消息的平台事实：桥照驱动报上来的填好交进来（18 第七节那张流程图的第二格）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// 场所编号（[`Venue::id`](crate::Venue::id) 拼），照原样进抽样的种子（「怎么走」第 6 条）。
    pub venue: VenueId,
    /// 这条消息：它在场所主线会话日志里的序号（`chat.md` 第七条第 1 条），进抽样的种子；顶替、承诺也认它。
    pub msg: Seq,
    /// 谁发的、他是谁、是不是冲她来的：和进站链共用一份（[`Said`]）。终端管理员、白名单成员冲她来的不过判官（[`route`]）。
    pub said: Said,
    /// @ 了别人没有：@ 了别人的不算续聊。
    pub mentions_others: bool,
    /// 引用的是不是别人的消息：引用别人的不算续聊。
    pub quotes_other: bool,
    /// 是不是只有表情，没有字：只有表情的不算刚说过话。
    pub textless: bool,
    /// 是不是只有图：只有图的不抽样。
    pub media_only: bool,
}

/// 她在这个场所真发出的一轮回复，外面从场所会话的日志（`venue.delivered`）投影出来交进来。一轮拆成几段发也只算一轮
/// （施工时定的第 4 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// 发出的时刻：这一轮第一条发出的时刻。
    pub at: Timestamp,
    /// 回的是谁：平台上的人的编号，一轮可以回几个人（第七条第 4 条的投影，几条的并集）；没有明确回谁的是空的，不算谁的
    /// 续聊。
    pub to: Vec<ExternalId>,
}

/// 主动回复判断的参数，代码里不写默认值（施工时定的第 5 条）：从 [`Params::chatty`](crate::Params::chatty) 拿，出厂的数
/// 在出厂文件里（`chat.md` 第八条，旧版的值见 18 第七节「旧版的默认值」），场所规则能按场所改。
///
/// 格只在 crate 里可见，外面造不出：拿到的每一格都照第八条的声明查过，不会有半衰期是 0、`k` 是 0 这类算出 NaN 的值
/// （自查第 14 条，施工 O-15）。
#[derive(Debug, Clone, PartialEq)]
pub struct Chatty {
    /// 抽样的千分比：`50` 是 5%，`0` 永不中，`1000` 及以上必中。
    pub(crate) probability: u16,
    /// 基础门槛：冷静抬高之前的门槛，旧版 `0.8`。
    pub(crate) base: f64,
    /// 五维的权重，照相关、意愿、社交、时机、连贯的先后，和 [`Judgement::scores`] 一一对上。全是 0 的，`raw` 是 0。
    pub(crate) weights: [f64; 5],
    /// `should_reply` 的调整：是真加它，是假减它；`0` 就是关了。
    pub(crate) adjust: f64,
    /// 冲她来的加分。
    pub(crate) direct: f64,
    /// 续聊的加分和窗口。
    pub(crate) continuation: Window,
    /// 刚说过话的加分和窗口。
    pub(crate) after_speaking: Window,
    /// 冷静的开关和曲线。
    pub(crate) restraint: Restraint,
    /// 违规的门槛：判官给的严重程度不低于它，不管分数够不够都回。
    pub(crate) severity_min: u8,
}

/// 一个带窗口的加值项的参数：[`Chatty`] 的一格，格只在 crate 里可见（施工 O-15）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    /// 成立时加多少分。
    pub(crate) bonus: f64,
    /// 窗口多少毫秒：离她最近一轮回复 `0 ≤ now − at < window` 才算（施工时定的第 2 条）。
    pub(crate) window: i64,
}

/// 冷静的参数（「怎么走」第 10 条）：抬 `cap × p³ ÷ (p³ + k³)`。曲线只留开关，不上界面（18 第七节）。[`Chatty`] 的一格，
/// 格只在 crate 里可见：半衰期、`k` 不会是 0（施工 O-15）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Restraint {
    /// 开没开：关着的不抬。
    pub(crate) on: bool,
    /// 一轮回复的分量衰减一半要多少毫秒，出厂 3 分钟。
    pub(crate) half_life: i64,
    /// 最多抬多少，出厂 `0.35`。
    pub(crate) cap: f64,
    /// 抬到一半时的近期发言量，出厂 `2.5`。
    pub(crate) k: f64,
}

/// 触发条件的种类：每个加值项一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 冲她来：私聊里的每一条，群里 @ 她、引用她的消息、以触发词开头的（[`addressed()`](crate::addressed())）。免冷静。
    Direct,
    /// 续聊：她刚回过这个人，他接着说。
    Continuation,
    /// 刚说过话：她刚在场所里说过话，任何人接着说。
    AfterSpeaking,
    /// 抽样：别的都不成立时照千分比抽中。
    Probability,
    /// 违规旗：进站链插的旗，只让判官认真查一眼。
    Moderation,
}

/// 一个成立了的条件：哪一种、加多少分。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// 哪一种。
    pub kind: Kind,
    /// 加多少分。
    pub bonus: f64,
}

/// 一条消息成立了的触发条件，是集合不是梯子：成立的都在，加分相加，不封顶（18 第七节）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Conditions {
    /// 成立了的条件，照插槽的先后（冲她来、续聊、刚说过话、违规旗、抽样）。顶替合起来的（[`supersede()`]），前一条的在前，
    /// 这一条新添的种类跟在后面。
    pub hits: Vec<Hit>,
}

impl Conditions {
    /// 主触发：照冲她来、续聊、刚说过话、抽样、违规旗的先后取第一个成立的（「怎么走」第 7 条）；一个都没有是 `None`。
    /// 只用来归类：日志、贴不贴表情、回合开头那句「为什么叫你」写哪一种。注意抽样排在违规旗前面，和插槽的先后不一样。
    pub fn primary(&self) -> Option<Kind> {
        [
            Kind::Direct,
            Kind::Continuation,
            Kind::AfterSpeaking,
            Kind::Probability,
            Kind::Moderation,
        ]
        .into_iter()
        .find(|&kind| self.has(kind))
    }

    /// 有没有这一种。
    fn has(&self, kind: Kind) -> bool {
        self.hits.iter().any(|hit| hit.kind == kind)
    }

    /// 只有违规旗：判官只查违规，不打分。
    fn only_moderation(&self) -> bool {
        !self.hits.is_empty() && self.hits.iter().all(|hit| hit.kind == Kind::Moderation)
    }
}

/// 加值项看一条消息时能看到的。
#[derive(Debug, Clone, Copy)]
pub struct BonusCtx<'a> {
    /// 这条消息的平台事实。
    pub facts: &'a Facts,
    /// 进站链插的旗。
    pub flags: &'a [Flag],
    /// 她在这个场所最近的回复，先后不要紧。
    pub replies: &'a [Reply],
    /// 此刻。
    pub clock: Clock,
    /// 参数。
    pub chatty: &'a Chatty,
}

/// 加值项的插槽（18 第十四节）：看一条消息成不成立，成立了给一笔。扩展加加值项也照它写。
///
/// 只看交进来的，不碰 I/O、时钟、随机源：同样的输入给出同样的回答。没有名字：成立的那一笔的 [`Kind`] 已经说清是哪一项
/// （第二条施工时定的第 14 条）。
pub trait Bonus {
    /// 这条消息成不成立；`before` 是插槽里排在前面、已经成立了的。
    fn judge(&self, ctx: &BonusCtx<'_>, before: &[Hit]) -> Option<Hit>;
}

/// 门槛修正看一次算分时能看到的。
#[derive(Debug, Clone, Copy)]
pub struct LiftCtx<'a> {
    /// 判官的回答。
    pub judgement: &'a Judgement,
    /// 成立了的条件。
    pub conditions: &'a Conditions,
    /// 她在这个场所最近的回复，先后不要紧。
    pub replies: &'a [Reply],
    /// 此刻。
    pub clock: Clock,
    /// 参数。
    pub chatty: &'a Chatty,
}

/// 门槛修正的插槽（18 第十四节）：给出门槛抬多少（负的是压低）。扩展加修正也照它写。
///
/// 只看交进来的，不碰 I/O、时钟、随机源。
pub trait Lift {
    /// 门槛抬多少。
    fn lift(&self, ctx: &LiftCtx<'_>) -> f64;
}

/// 自带的加值项，照这个先后：冲她来、续聊、刚说过话、违规旗、抽样（「怎么走」第 1 条）。抽样排最后，它要看前面的。
fn bonuses() -> [&'static dyn Bonus; 5] {
    [
        &direct::Item,
        &continuation::Item,
        &after_speaking::Item,
        &moderation::Item,
        &probability::Item,
    ]
}

/// 自带的门槛修正：只有冷静（「怎么走」第 10 条）。
fn lifts() -> [&'static dyn Lift; 1] {
    [&restraint::Item]
}

/// 算一条消息成立了哪些触发条件：自带的加值项照先后过，成立的都记下（「怎么走」第 1 到 6 条）。`flags` 是进站链插的旗，
/// `replies` 是她在这个场所最近的回复，先后不要紧。
pub fn conditions(
    facts: &Facts,
    flags: &[Flag],
    replies: &[Reply],
    clock: Clock,
    chatty: &Chatty,
) -> Conditions {
    let ctx = BonusCtx {
        facts,
        flags,
        replies,
        clock,
        chatty,
    };
    let mut hits = Vec::new();
    for bonus in bonuses() {
        if let Some(hit) = bonus.judge(&ctx, &hits) {
            hits.push(hit);
        }
    }
    Conditions { hits }
}

/// 一条消息走哪条路（「怎么走」第 8 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// 一个条件都没有：只记下。
    Record,
    /// 终端管理员、白名单成员冲她来：直接回，不过判官（Q4；白名单成员施工 O-27，2026-10-10 项目主人定）。
    Commit,
    /// 只有违规旗：判官只查违规。
    ModerationOnly,
    /// 交给判官打分。别的人的 @ 也走这条。
    Judge,
}

/// 一条消息走哪条路：没有条件只记下；终端管理员、白名单成员冲她来直接回；只有违规旗判官只查违规；别的交给判官打分（「怎么走」
/// 第 8 条）。
pub fn route(conditions: &Conditions, standing: Standing) -> Route {
    let known = matches!(standing, Standing::Admin | Standing::Whitelisted);
    if conditions.hits.is_empty() {
        Route::Record
    } else if conditions.has(Kind::Direct) && known {
        Route::Commit
    } else if conditions.only_moderation() {
        Route::ModerationOnly
    } else {
        Route::Judge
    }
}

/// 她不晚于此刻的回复里最近的一轮：同一毫秒的几轮取交进来靠后的那一轮。晚于此刻的不看，交多了不要紧。
fn latest(replies: &[Reply], now: Timestamp) -> Option<&Reply> {
    replies
        .iter()
        .filter(|reply| reply.at <= now)
        .max_by_key(|reply| reply.at)
}

/// `at` 落在此刻的窗口里没有：`0 ≤ now − at < window`（毫秒），正好 `window` 以前的不算（施工时定的第 2 条）。
fn within(at: Timestamp, now: Timestamp, window: i64) -> bool {
    // 时刻只到 9999 年，相减不会溢出。
    let age = now.unix_millis() - at.unix_millis();
    (0..window).contains(&age)
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
