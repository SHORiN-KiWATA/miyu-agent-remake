//! 冻结在会话上的策略（`docs/designs/02-内核.md` K3，第六节「回合怎么开、请求怎么发」第 7 条），和回合开始时换上新的一份
//! （施工 P-1 再补，`docs/blueprint/kernel/session.md`「换策略快照」）。

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;

use crate::assemble::Assembler;
use crate::estimate::Flat;
use crate::event::{Body, Event, PolicyChanged};
use crate::facts::FactTemplates;
use crate::id::{CommandId, ContentHash};
use crate::origin::By;
use crate::template::Template;
use crate::time::Timestamp;
use crate::tool::{ToolRule, ToolTexts};

use super::Session;

/// 冻结在会话上的策略：造会话时由执行器照策略快照造好交进来（施工 3-6），会话里不再变。
pub struct Policy {
    /// 组装请求的做法，一个会话一种（`05-内核接口.md` 第五节「组装请求」）。会话是一个 actor，
    /// 会被挪到别的线程上跑（02 第七节），所以要 `Send`。
    pub assembler: Box<dyn Assembler + Send>,
    /// 事实的模板：环境、权限、会话编号，和回复被截断的那一句（`08-上下文投影.md` 第五节「环境和状态的事实怎么写」）。
    pub facts: FactTemplates,
    /// 工具面上每件工具的访问类别和参数格式，照名字查。不在这里的名字是模型编的。
    pub tools: BTreeMap<String, ToolRule>,
    /// 一个回合最多请求几次模型；没有就是不限（`02-内核.md` 第六节「工具怎么调、下一步怎么走」）。
    pub step_limit: Option<u32>,
    /// 内核替工具写给模型的那几句。
    pub tool_texts: ToolTexts,
    /// 有没有人能确认：没有确认界面的场所没有，`miyu ask` 一律没有（`22-命令行.md` 第三节）。没有的，执行前的链说
    /// 要问人时当场拒绝（`02-内核.md` 第六节「确认怎么走」第 2 条）。
    pub attended: bool,
    /// 有计划的重启打断了一轮，再起来时连着接着干几次；接够了还被打断，就等人开口（`02-内核.md`
    /// 第六节「载入、崩溃、重启」第 4 条，初值 3）。
    pub resumes: u32,
    /// 压缩用的数；没有的不主动压（`compaction.md`，施工 6-2 上）。
    pub compaction: Option<Compaction>,
    /// 检查点里代码写的几段的模板（`compaction.md` 第八条，施工 6-5）；没有的不写那几段。
    pub notes: Option<Notes>,
    /// 子会话向上回报的正文怎么截（施工 7-6，`report.rs`）。
    pub reports: Reports,
    /// 起标题的两个数（施工 3-8 五补，`title.rs`）；没有的不起标题：以前造的快照里没有。
    pub titles: Option<Titles>,
    /// 别的会话发来的话怎么防刷屏（施工 C-2，`peers.rs`）。以前造的快照里没有的，执行器照出厂的数交进来：防刷屏不能因为
    /// 会话旧就不管（`docs/blueprint/cross-session.md`「对外的样子」）。
    pub peers: Peers,
}

/// 起标题的两个数（施工 3-8 五补，`docs/blueprint/kernel/session.md`「起标题」）：数值是数据，放在策略快照里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Titles {
    /// 一个会话最多试几次，都没起成就不再试：出厂 2。
    pub tries: u32,
    /// 标题最多几个字（Unicode 字符），超了截掉：出厂 50（2026-10-01 项目主人定）。
    pub chars: usize,
}

/// 别的会话发来的话怎么防刷屏（施工 C-2，`docs/blueprint/cross-session.md` 第五条第 1 到 3 款），「空了告诉我」等多久、
/// 通知带多长的一行（施工 C-6，第六条第 6、8 款）。数是策略数据，放在策略快照里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peers {
    /// 同一个发话方在一个窗口里最多几句：策略数据 `peers.burst`，出厂 5。
    pub burst: usize,
    /// 限速、去重看多久以内的，单位秒：策略数据 `peers.window`，出厂 600。含正好这么久以前的那一刻。
    pub window: u64,
    /// 还没听到的别的会话的话最多几句，不分发话方：策略数据 `peers.unread`，出厂 50。
    pub unread: usize,
    /// 订了多久没等到通知就作废，单位小时：策略数据 `peers.watch_hours`，出厂 12（施工 C-6）。等的这一边照它查到没到点。
    pub watch_hours: u64,
    /// 通知里带的那一行最多几个字（Unicode 字符）：策略数据 `peers.status_chars`，出厂 200（施工 C-6）。被等的这一边照它
    /// 截 [`super::Session::last_line`]。
    pub status_chars: usize,
}

/// 子会话向上回报的正文怎么截（施工 7-6，`docs/blueprint/agents.md` 第二条第 3 条）：超过 `chars` 个字的留头尾各一半，
/// 中间接一行 `omitted`。数和字都是数据，放在策略快照里。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reports {
    /// 正文最多几个字（Unicode 字符）：策略数据 `jobs.report_chars`，出厂 30000。
    pub chars: usize,
    /// 截在中间的那一行：字段 `count` 是省掉的字数（`core/jobs/subagent-omitted.txt`）。以前造的快照没有，是空的：头尾
    /// 之间只空一行。
    pub omitted: Template,
}

/// 检查点里代码写的几段的模板（施工 6-5）：`compaction/notes-*.txt`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notes {
    /// 读过、改过的文件清单的头一行；下面一个一行由内核写。
    pub files: Template,
    /// 清单放不下的还有几个：字段 `count`。
    pub files_more: Template,
    /// 取回指路：字段 `upto`。
    pub retrieve: Template,
    /// 太大没重读的：字段 `files`。
    pub too_large: Template,
    /// 待办那一段的头一行；下面一项一行由内核写（施工 D-3）。以前造的快照里没有的不写这一段。
    pub todos: Option<Template>,
    /// 摘要请求截短过的，摘要没看到的那一段：字段 `from`、`to`（施工 6-6 中）。没有的不写。
    pub uncovered: Option<Template>,
}

/// 压后重建的数（`compaction.md` 第九条，施工 6-5）。数值是数据，放在策略快照里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rebuild {
    /// 最多重读几个。出厂 5。
    pub files: usize,
    /// 单个最多多少 token，超了只进清单。出厂 5000。
    pub file_tokens: u64,
    /// 合计最多多少 token。出厂 50000。
    pub total: u64,
    /// 窗口不到这么多的不重读，只写清单和取回指路。出厂 32000。
    pub min_window: u64,
    /// 交给执行器的候选最多几个：有读不到、太大的，挑满要留余地。出厂 10。
    pub candidates: usize,
}

/// 压缩用的数（`compaction.md`「对外的样子」的策略数据）。数值是数据，放在策略快照里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Compaction {
    /// 输出预留的上限：输出预留 = min(模型的最大输出, 它)。出厂 20000。
    pub reserve_cap: u64,
    /// 余量：压缩线离「放不下」还空多少。出厂 13000。
    pub margin: u64,
    /// 尾巴的预算上限：压完原样留着的最近一段，至多这么多 token，也不超过压缩线的四分之一。出厂 16000（施工 6-2 下）。
    pub tail: u64,
    /// 本地估算时一张图、一个文件各算多少 token。出厂各 2000。
    pub price: Flat,
    /// 压后重建的数（施工 6-5）；没有的不重读。
    pub rebuild: Option<Rebuild>,
    /// 熔断的数（施工 6-6 上）；没有的不熔断。
    pub pause: Option<Pause>,
    /// 摘要请求超长时截短再试的数（施工 6-6 中）；没有的不截，照失败算。
    pub shorten: Option<Shorten>,
    /// fork 式的摘要回复里调了工具，改走隔离式（施工 6-6 下）。以前的快照没有隔离式那句 system，是假：照失败算。
    pub isolate: bool,
}

/// 摘要请求超长时截短再试的数（`compaction.md` 第三条第 10 条，施工 6-6 中）。数值是数据，放在策略快照里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shorten {
    /// 截着最多再试几次。出厂 3。
    pub tries: u32,
    /// 供应商没说超了多少时，去掉剩下的组的百分之几（向上取整、至少一组）。出厂 20。
    pub percent: u32,
}

/// 熔断的数（`compaction.md` 第十条，施工 6-6 上）。数值是数据，放在策略快照里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pause {
    /// 自动压缩连续失败几次就暂停。出厂 3。
    pub failures: u32,
    /// 上一个检查点所在的那一轮算第 1 个回合，第几个回合以内又到线算「很快」。出厂 3。
    pub turns: u32,
    /// 连着很快又到线几次就暂停：到了这一次不压，改写暂停。出厂 3。
    pub refills: u32,
}

/// 组装器是外面交进来的，不一定能打印，跳过它。
impl fmt::Debug for Policy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Policy")
            .field("facts", &self.facts)
            .field("tools", &self.tools)
            .field("step_limit", &self.step_limit)
            .field("tool_texts", &self.tool_texts)
            .field("attended", &self.attended)
            .field("resumes", &self.resumes)
            .field("compaction", &self.compaction)
            .field("notes", &self.notes)
            .field("reports", &self.reports)
            .field("peers", &self.peers)
            .finish_non_exhaustive()
    }
}

/// 会话手里的策略（施工 P-1 再补）：现在用的那一份，和执行器先放着、回合开始的挂接点跑完时换上的新的一份。解引用成
/// 现在用的那一份。
#[derive(Debug)]
pub(super) struct Held {
    current: Policy,
    staged: Option<Policy>,
}

impl Held {
    /// 造会话、载入时交进来的那一份。
    pub(super) fn new(policy: Policy) -> Held {
        Held {
            current: policy,
            staged: None,
        }
    }
}

impl Deref for Held {
    type Target = Policy;

    fn deref(&self) -> &Policy {
        &self.current
    }
}

impl Session {
    /// 先放着一份新的策略（施工 P-1 再补）：人格的文件改了，执行器照新的拼好快照、造好策略，在交回这一轮的
    /// [`super::Input::TurnStartHooksDone`] 以前交进来；那一条带着新快照的哈希时换上，没带的扔掉。再交一份盖掉前一份。
    pub fn stage_policy(&mut self, policy: Policy) {
        self.policy.staged = Some(policy);
    }

    /// 回合开始的挂接点跑完了：带着新快照的哈希 `hash`、又有放着的那一份的，换上它，记一条 `session.policy_changed`（只写
    /// `policy`，`by` 是内核，`cause` 是回合的）。回合开头的事实是照旧的那一份查的：照新的再查一次角色扮演提示，到了轮数
    /// 的跟着记（旧人格没有提示、新的有，这一轮就带上）。缺一样的什么都不记，放着的扔掉。
    pub(super) fn swap_policy(
        &mut self,
        at: Timestamp,
        cause: Option<CommandId>,
        hash: Option<ContentHash>,
    ) -> Vec<Event> {
        let staged = self.policy.staged.take();
        let (Some(hash), Some(policy)) = (hash, staged) else {
            return Vec::new();
        };
        self.policy.current = policy;
        let body = Body::PolicyChanged(PolicyChanged {
            policy: Some(hash),
            ..PolicyChanged::default()
        });
        let mut events = vec![self.record(at, By::Kernel, cause.clone(), body)];
        if let Some(fact) = self.policy.facts.reminder(&self.history) {
            events.push(self.record(at, By::Kernel, cause, Body::ContextInjected(fact)));
        }
        events
    }
}
