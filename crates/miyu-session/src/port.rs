//! 请求模型的端口（`02-内核.md` 第四节「执行器怎么回动作」里「请求模型」那一行）：actor 把请求交给
//! 它，它的回报送回 actor 的收件箱。3-7（下）接上驱动和 HTTP 执行器，以后资源调度夹在中间；测试里
//! 照剧本回。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use miyu_drivers::DriverTexts;
use miyu_kernel::accumulate::Delta;
use miyu_kernel::event::{CallError, Cost, EffortInUse, Purpose, Usage};
use miyu_kernel::id::{AccountId, ContentHash, Seq, SessionId};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::{Limits, Replaced};
use miyu_models::price::Tariff;
use miyu_store::blob::Blobs;

use crate::config::TurnConfig;
use crate::route::OneShot;

/// 给一个会话造请求模型的端口（施工 3-7 下）。造会话、载入时，拿到了这个会话的策略快照再造：驱动的
/// 占位冻结在快照里，核心升级改了字，老会话照样逐字节重现当时的请求（施工 3-6 上）。
pub trait Models: Send + Sync {
    /// 等造端口要的都备好了（施工 8-7：目录在写了 `ready` 以后才读完）。造会话、载入时在造端口之前等它。默认马上好。
    fn ready(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(std::future::ready(()))
    }

    /// 造这个会话的端口。
    fn port(&self, session: ForSession) -> Arc<dyn ModelPort>;

    /// 模型调用口的一次性入口（施工 8-20，`docs/blueprint/models.md`「怎么走」第十二条）：发一次、拿整段回答，和会话的
    /// 端口共用一份底子（冷却表、池的指针）。路由交它自己；照剧本回的（测试的端口）没有。
    fn one_shot(&self) -> Option<OneShot> {
        None
    }
}

/// 造端口时交进来的，这个会话自己的。
#[derive(Debug, Clone)]
pub struct ForSession {
    /// 会话编号：路由照它挑 key（施工 8-6，`models.md` 第一条第 6 条）。
    pub id: SessionId,
    /// 会话的属主（施工 8-15）：替它看图的一次性调用记在他的账上。
    pub owner: AccountId,
    /// 造会话、载入时取的那一份配置（施工 8-6）：会话用哪个模型、限额照它定。
    pub config: TurnConfig,
    /// 驱动的占位：取自这个会话的策略快照。
    pub texts: DriverTexts,
    /// 属主的 blob：编码要用的图、文件在这里。
    pub blobs: Blobs,
    /// 会话记着的引用（施工 8-8）：模型或 `@池`。造会话的是 `session.created` 的 `model`；载入的是内核从日志算的
    /// （`Session::reference()`，施工 8-10：被后来带 `model` 的 `session.policy_changed` 盖掉）。以前的日志没有这一格的、
    /// 造的时候连 `models.chat` 都没配的是空的：照造端口这一刻的 `models.chat`。
    pub reference: Option<String>,
    /// 最近一条发出去了的 `model.called` 发给了谁（施工 8-8，「起草时定的」第 2 条）：引用是钉住的池的，照它认钉着的成员。
    /// 新造的会话、一次都没发出去过的没有。
    pub sent: Option<Model>,
}

/// 请求模型的端口。
pub trait ModelPort: Send + Sync {
    /// 发给哪个端点的哪个模型：记进运行日志的 `request` 那一行。路由的是这个会话钉着的、上一次解析出来的那一个（施工 8-6：
    /// 回合开始时照新的配置可能换，真发给谁记在 `model.called` 里）。
    fn model(&self) -> Model;

    /// 会话这时生效的引用（施工 8-8）：模型或 `@池`。派子代理不写池时，子会话记下它（`models.md` 第三条第 4 条）。路由的是
    /// 钉着的那一个；不知道的（测试的端口）没有。
    fn reference(&self) -> Option<String> {
        None
    }

    /// 回合开始（施工 8-10，`models.md`「怎么走」第六条第 3 条）：照这一轮的配置 `config` 重新解析会话的引用 `reference`
    /// （内核交的，以前的会话没有的照端口自己记着的）；端点、限额跟着换。钉着的没了、退回了这一轮的 `models.chat` 的，
    /// 交回原来的和退回的，内核记下。不重新解析的（测试的端口）什么都不做。
    fn turn(&self, _config: &TurnConfig, _reference: Option<&str>) -> Option<Replaced> {
        None
    }

    /// 接下来那个模型真用的思考强度（施工 8-18，`models.md`「怎么走」第十一条第 7 条）：给头看的，`subscribe`、
    /// `model.changed` 照它写。轮换的池、什么都不带的、不知道的（测试的端口）没有。
    fn effort(&self) -> Option<EffortInUse> {
        None
    }

    /// 这个模型的限额：窗口、最大输出、一张图怎么算（施工 6-3 上）。会话 actor 造会话、载入以后交给内核。不知道的
    /// 都是没有：不主动压。
    fn limits(&self) -> Limits {
        Limits {
            model: self.model(),
            window: None,
            max_output: None,
            images: None,
            blind: false,
        }
    }

    /// 替看不了图的模型看图（施工 8-17，`docs/blueprint/models.md`「怎么走」第十三条第 4 条）：照这一轮的配置 `config` 把
    /// `request` 经一次性入口发给 `models.vision`，说完了交给 `sight`。马上返回，在别的任务里发，带上当前的 span。没有一次性
    /// 入口的（测试的端口）当场交没成。
    fn describe(&self, request: Request, config: &TurnConfig, sight: Sight) {
        let _ = (request, config);
        sight.unseen("this model port cannot describe images".to_string());
    }

    /// 发一次请求。马上返回，在别的任务里发：actor 不等它。`config` 是这一轮的配置（回合开始时冻结的，施工 8-4）：这一轮
    /// 的每一次请求都照它，中途改了配置下一轮才用上。
    ///
    /// 回报照先后交给 `reports`：先报发出去了，再一段段交增量，最后报说完了；没发出去就失败了的，
    /// 直接报说完了。`cancel` 叫停了就停下，什么都不再报：会话不要这次请求了，或者会话停了。在别的
    /// 任务里发的，带上当前的 span（`tracing::Span::current()`），发出来的日志才带着会话编号。
    fn call(
        &self,
        seen: Seq,
        request: Request,
        config: &TurnConfig,
        reports: Reports,
        cancel: Cancel,
    );
}

/// 一次请求的回报送回哪里。
#[derive(Debug)]
pub struct Reports {
    seen: Seq,
    /// 辅助请求的用途（施工 3-8 四补的回顾、五补的起标题）：回报另走一路，不和主请求的 `seen` 撞。主请求没有。
    purpose: Option<Purpose>,
    /// 这一次真发给的模型的价格（施工 8-15，[`Reports::billed`]）：说完了照用量算金额。没有的不算。
    tariff: Option<Tariff>,
    back: mpsc::UnboundedSender<Back>,
}

impl Reports {
    pub(crate) fn new(seen: Seq, back: mpsc::UnboundedSender<Back>) -> Reports {
        Reports {
            seen,
            purpose: None,
            tariff: None,
            back,
        }
    }

    /// 辅助请求的回报（施工 3-8 四补、五补）：名字是用途和它照到的那一条 `upto`。
    pub(crate) fn aside(purpose: Purpose, upto: Seq, back: mpsc::UnboundedSender<Back>) -> Reports {
        Reports {
            seen: upto,
            purpose: Some(purpose),
            tariff: None,
            back,
        }
    }

    /// 这一次照 `tariff` 算金额（施工 8-15，`docs/blueprint/models.md`「怎么走」第九条第 3 条）：端口挑定了真发给的模型以后
    /// 交，说完了照报的用量算好，随说完了交给内核。不交的（测试的剧本）不算，`model.called` 不写 `cost`。
    #[must_use]
    pub fn billed(mut self, tariff: Option<Tariff>) -> Reports {
        self.tariff = tariff;
        self
    }

    /// 是哪一种辅助请求；主请求没有（施工 3-8 五补）。端口照请求发，用不着它；测试的端口照它分剧本。
    pub fn purpose(&self) -> Option<&Purpose> {
        self.purpose.as_ref()
    }

    /// 发出去了：发给了哪个模型，请求字节的哈希。
    pub fn sent(&self, model: Model, request: ContentHash) {
        self.send(Report::Sent { model, request });
    }

    /// 一段增量。
    pub fn delta(&self, delta: Delta) {
        self.send(Report::Delta(delta));
    }

    /// 说完了：正常说完的带用量，出错的带分类和原话，供应商说了要等多久的带上毫秒数，超长的带上超了多少 token
    /// （施工 6-6 中）。
    pub fn ended(
        self,
        usage: Option<Usage>,
        error: Option<CallError>,
        wait_ms: Option<u64>,
        excess: Option<u64>,
    ) {
        let cost = self.cost(usage.as_ref());
        self.send(Report::Ended {
            usage,
            cost,
            error,
            wait_ms,
            excess,
            failover: false,
        });
    }

    /// 出错了，端口换了端点（施工 8-9，`models.md`「怎么走」第五条第 3 条）：内核不管分类当场再来。`wait_ms` 是别的候选都在
    /// 冷却时要等多久，有别的能用的没有。
    pub fn failed_over(self, usage: Option<Usage>, error: CallError, wait_ms: Option<u64>) {
        let cost = self.cost(usage.as_ref());
        self.send(Report::Ended {
            usage,
            cost,
            error: Some(error),
            wait_ms,
            excess: None,
            failover: true,
        });
    }

    /// 照交来的价格和报的用量算金额：没有价格、没报用量、算不出的没有。
    fn cost(&self, usage: Option<&Usage>) -> Option<Box<Cost>> {
        self.tariff.as_ref()?.cost(usage?).map(Box::new)
    }

    #[expect(
        clippy::let_underscore_must_use,
        reason = "会话停了就送不进去：回报没人要了，丢掉"
    )]
    fn send(&self, report: Report) {
        let back = match &self.purpose {
            Some(purpose) => Back::Aside {
                purpose: purpose.clone(),
                upto: self.seen,
                report,
            },
            None => Back::Report {
                seen: self.seen,
                report,
            },
        };
        let _ = self.back.send(back);
    }
}

/// 一次转述的结果送回哪里（施工 8-17）：成了的交替它看的模型和转述，没成的交为什么。都送回 actor 的收件箱。
#[derive(Debug)]
pub struct Sight {
    blob: ContentHash,
    back: mpsc::UnboundedSender<Back>,
}

impl Sight {
    pub(crate) fn new(blob: ContentHash, back: mpsc::UnboundedSender<Back>) -> Sight {
        Sight { blob, back }
    }

    /// 转述成了：替它看的是 `model`，转述是 `text`（去掉了前后空白，不是空的）。
    pub fn seen(self, model: Model, text: String) {
        self.send(Ok((model, text)));
    }

    /// 转述没成：为什么，记进运行日志。
    pub fn unseen(self, why: String) {
        self.send(Err(why));
    }

    #[expect(
        clippy::let_underscore_must_use,
        reason = "会话停了就送不进去：转述没人要了，丢掉"
    )]
    fn send(self, seen: Result<(Model, String), String>) {
        let _ = self.back.send(Back::Described {
            blob: self.blob,
            seen,
        });
    }
}

/// 叫停一次请求：会话不要这次请求了，或者会话停了（actor 放下了叫停的那一头）。会话停了就没人要
/// 结果了，接着读只是白花 token。说完了以后放下的，请求已经不在读了，停不停都一样。
#[derive(Debug)]
pub struct Cancel(oneshot::Receiver<()>);

impl Cancel {
    pub(crate) fn new(receiver: oneshot::Receiver<()>) -> Cancel {
        Cancel(receiver)
    }

    /// 等到被叫停。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "明着叫停和放下了一个意思：都是叫停"
    )]
    pub async fn wait(self) {
        let _ = self.0.await;
    }
}

/// 请求的一样回报。
#[derive(Debug)]
pub(crate) enum Report {
    /// 发出去了。
    Sent { model: Model, request: ContentHash },
    /// 一段增量。
    Delta(Delta),
    /// 说完了。`failover`：出错以后端口换了端点（施工 8-9）。
    Ended {
        usage: Option<Usage>,
        /// 装在盒子里：照 `model.called` 的那一格，回报不平白变大。
        cost: Option<Box<Cost>>,
        error: Option<CallError>,
        wait_ms: Option<u64>,
        excess: Option<u64>,
        failover: bool,
    },
}

/// 执行器送回 actor 的：请求的回报、到点了、工具的回报。
#[derive(Debug)]
pub(crate) enum Back {
    /// 请求 `seen` 的一样回报。
    Report { seen: Seq, report: Report },
    /// 辅助请求（用途 `purpose`、照到 `upto`）的一样回报（施工 3-8 四补、五补）。
    Aside {
        purpose: Purpose,
        upto: Seq,
        report: Report,
    },
    /// 为请求 `seen` 等的时刻到了。
    Woke { seen: Seq },
    /// 跑工具的任务送回来的（施工 4-2）。
    Tool(crate::tools::ToolBack),
    /// 一条后台命令结束了（施工 7-3）。
    Job(crate::jobs::Ended),
    /// 替它看图回来了（施工 8-17）：哪一张图，成了的替它看的模型和转述，没成的为什么。
    Described {
        blob: ContentHash,
        seen: Result<(Model, String), String>,
    },
    /// 等会话 `session` 等不到了（施工 C-6，`peers.rs`）：到点了是 `expired`，订的时候它不在了是 `gone`。
    WatchEnded {
        session: miyu_kernel::id::SessionId,
        reason: miyu_kernel::event::IdleReason,
    },
}
