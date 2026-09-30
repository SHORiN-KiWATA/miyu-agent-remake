//! 请求模型的端口（`02-内核.md` 第四节「执行器怎么回动作」里「请求模型」那一行）：actor 把请求交给
//! 它，它的回报送回 actor 的收件箱。3-7（下）接上驱动和 HTTP 执行器，以后资源调度夹在中间；测试里
//! 照剧本回。

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use miyu_drivers::DriverTexts;
use miyu_kernel::accumulate::Delta;
use miyu_kernel::event::{CallError, Purpose, Usage};
use miyu_kernel::id::{ContentHash, Seq};
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::Limits;
use miyu_store::blob::Blobs;

/// 给一个会话造请求模型的端口（施工 3-7 下）。造会话、载入时，拿到了这个会话的策略快照再造：驱动的
/// 占位冻结在快照里，核心升级改了字，老会话照样逐字节重现当时的请求（施工 3-6 上）。
pub trait Models: Send + Sync {
    /// 造这个会话的端口。
    fn port(&self, session: ForSession) -> Arc<dyn ModelPort>;
}

/// 造端口时交进来的，这个会话自己的。
#[derive(Debug, Clone)]
pub struct ForSession {
    /// 驱动的占位：取自这个会话的策略快照。
    pub texts: DriverTexts,
    /// 属主的 blob：编码要用的图、文件在这里。
    pub blobs: Blobs,
}

/// 请求模型的端口。
pub trait ModelPort: Send + Sync {
    /// 发给哪个端点的哪个模型：记进运行日志的 `request` 那一行。
    fn model(&self) -> &Model;

    /// 这个模型的限额：窗口、最大输出、一张图怎么算（施工 6-3 上）。会话 actor 造会话、载入以后交给内核。不知道的
    /// 都是没有：不主动压。
    fn limits(&self) -> Limits {
        Limits {
            model: self.model().clone(),
            window: None,
            max_output: None,
            images: None,
        }
    }

    /// 发一次请求。马上返回，在别的任务里发：actor 不等它。
    ///
    /// 回报照先后交给 `reports`：先报发出去了，再一段段交增量，最后报说完了；没发出去就失败了的，
    /// 直接报说完了。`cancel` 叫停了就停下，什么都不再报：会话不要这次请求了，或者会话停了。在别的
    /// 任务里发的，带上当前的 span（`tracing::Span::current()`），发出来的日志才带着会话编号。
    fn call(&self, seen: Seq, request: Request, reports: Reports, cancel: Cancel);
}

/// 一次请求的回报送回哪里。
#[derive(Debug)]
pub struct Reports {
    seen: Seq,
    /// 辅助请求的用途（施工 3-8 四补的回顾、五补的起标题）：回报另走一路，不和主请求的 `seen` 撞。主请求没有。
    purpose: Option<Purpose>,
    back: mpsc::UnboundedSender<Back>,
}

impl Reports {
    pub(crate) fn new(seen: Seq, back: mpsc::UnboundedSender<Back>) -> Reports {
        Reports {
            seen,
            purpose: None,
            back,
        }
    }

    /// 辅助请求的回报（施工 3-8 四补、五补）：名字是用途和它照到的那一条 `upto`。
    pub(crate) fn aside(purpose: Purpose, upto: Seq, back: mpsc::UnboundedSender<Back>) -> Reports {
        Reports {
            seen: upto,
            purpose: Some(purpose),
            back,
        }
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
        self.send(Report::Ended {
            usage,
            error,
            wait_ms,
            excess,
        });
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
    /// 说完了。
    Ended {
        usage: Option<Usage>,
        error: Option<CallError>,
        wait_ms: Option<u64>,
        excess: Option<u64>,
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
}
