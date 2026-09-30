//! 正文里的一条：种类、字、属于哪一轮，和各种条目自己带的东西。

use crate::core::{Level, Report};

use super::{Chip, Progress, Segment};

/// 正文里一条的种类，决定怎么画。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// 你说的话。
    User,
    /// 她的回答。
    Reply,
    /// 时间线的一段：思考、调工具。
    Steps,
    /// 打断了、撤销了这类旁白。
    Note,
    /// 出错了。
    Error,
    /// 一轮做完的收尾行：模型、用时、做完的时刻。
    Done,
    /// 核心断开时那一轮的收尾：和被打断的一个样子，整行红（蓝图「连核心」第 7 条）。
    Cut,
    /// 撤销了几轮：一行说明，点开看你说的那句的全文（`text`）。
    Undo,
    /// 一条后台任务结束的通知（`job` 里是记号和能点开看的全文）；抽屉了结以后的那一行（不允许、取消）也是它。
    Job,
    /// 提问答了：旧版的引用块，行首暗色竖线、整块暗色（蓝图「确认和提问的抽屉」第 6 条）。
    Answered,
}

/// 后台任务结束的通知：成败决定记号的颜色（蓝图「后台命令、子代理和侧边栏」第 5 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobMark {
    /// 做完了：绿 `●`。
    Done,
    /// 失败了：红 `✗`。
    Failed,
    /// 停掉了（连同抽屉取消）：暗 `●`，整行暗。
    Stopped,
}

/// 通知里的记号，和点开看的全文（子代理的报告；没有的是空的）。
#[derive(Debug, Clone)]
pub struct JobNote {
    /// 记号。
    pub mark: JobMark,
    /// 点开看的全文。
    pub detail: String,
    /// 哪条任务的（任务编号）；演示的、不是任务的是 `None`。
    pub source: Option<String>,
}

/// 正文里的一条。
#[derive(Debug, Clone)]
pub struct Entry {
    /// 生出来时发的编号，这次启动里不重复：排好的行按它记（`ui/row_cache`），不按它在正文里排第几。
    pub id: u64,
    /// 种类。
    pub kind: Kind,
    /// 字；时间线那一条没有字。
    pub text: String,
    /// 时间线那一条的一段；别的是 `None`。
    pub segment: Option<Segment>,
    /// 属于哪一轮；你刚说、还没开轮的，和撤销说明这类不属于哪一轮的，是 `None`。
    pub turn: Option<u64>,
    /// 这一轮被撤掉了：不画，恢复时再画。
    pub hidden: bool,
    /// 你说的话发出去时正在回答：先排着，列在运行状态行下面；开了它那一轮才进正文（`tui.md`「运行状态行和排队的消息」第 5 条）。
    pub queued: bool,
    /// 你说的话落盘以后的序号（`message.user`）：认它开了哪一轮、被退回的是哪一句。别的条是 `None`。
    pub seq: Option<u64>,
    /// 撤销那一行：核心回应里给人看的几样。别的条是 `None`。
    pub undo: Option<Report>,
    /// 人点开了：撤销那一行下面铺底色写全文。
    pub open: bool,
    /// 你说的话发出去那一刻的权限级别：竖线照它上色，之后换了级别也不变。别的条是 `None`。
    pub level: Option<Level>,
    /// 后台任务结束的通知：记号和全文。别的条是 `None`。
    pub job: Option<JobNote>,
    /// 别处来的话的来处（蓝图「别处来的话」第 2 条）：`从 B 收到消息` 这样一句；你在这个界面说的话是 `None`。
    pub from: Option<String>,
    /// 你说的话里的块：粘贴块点开看全文，附件不展开（蓝图「正文」第 2 条、「输入框」第 12 条）。别的条是空的。
    pub pasted: Vec<Chip>,
    /// 她的回答里点过的 `<details>`（第几个，从 0 数）：和它写的 `open` 反过来（蓝图「她的回答：Markdown」第 15 条）。
    pub details: Vec<usize>,
    /// 正在压缩的那一行的进度（蓝图「正文」第 9 条）。压好了、失败了是 `None`。
    pub progress: Option<Progress>,
    /// 旁白前面绿色的记号（压好了的 `● `，蓝图「正文」第 9 条）；别的是 `None`。
    pub mark: Option<String>,
}
