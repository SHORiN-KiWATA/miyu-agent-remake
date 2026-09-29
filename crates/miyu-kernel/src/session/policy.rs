//! 冻结在会话上的策略（`docs/designs/02-内核.md` K3，第六节「回合怎么开、请求怎么发」第 7 条）。

use std::collections::BTreeMap;
use std::fmt;

use crate::assemble::Assembler;
use crate::estimate::Flat;
use crate::facts::FactTemplates;
use crate::tool::{ToolRule, ToolTexts};

/// 冻结在会话上的策略：造会话时由执行器照策略快照造好交进来（施工 3-6），会话里不再变。
pub struct Policy {
    /// 组装请求的做法，一个会话一种（`05-内核接口.md` 第五节「组装请求」）。会话是一个 actor，
    /// 会被挪到别的线程上跑（02 第七节），所以要 `Send`。
    pub assembler: Box<dyn Assembler + Send>,
    /// 两类事实的模板：环境和权限（`08-上下文投影.md` 第五节「环境和状态的事实怎么写」）。
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
}

/// 压缩用的数（`compaction.md`「对外的样子」的策略数据）。数值是数据，放在策略快照里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Compaction {
    /// 输出预留的上限：输出预留 = min(模型的最大输出, 它)。出厂 20000。
    pub reserve_cap: u64,
    /// 余量：压缩线离「放不下」还空多少。出厂 13000。
    pub margin: u64,
    /// 本地估算时一张图、一个文件各算多少 token。出厂各 2000。
    pub price: Flat,
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
            .finish_non_exhaustive()
    }
}
