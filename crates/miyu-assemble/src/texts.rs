//! 给模型看的几句固定的字：检查点的包装、回合没走完的那一句、压缩的摘要指令（`docs/designs/08-上下文投影.md`
//! 第四节第 5 条，`26-提示词.md` 第八节）。
//!
//! 出厂的放在资源目录的 `core/` 下，由执行器读好交进来（施工 M3）；这里只管拿来拼，
//! 不读文件。文件的内容原样用，行尾的换行也算。

use miyu_kernel::event::EndReason;
use miyu_kernel::template::Template;

/// 组装时要用的几句固定的字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texts {
    /// 检查点包装的开头，摘要紧接在它后面（`core/checkpoint-open.txt`）。
    pub checkpoint_open: String,
    /// 摘要的收尾，紧接在摘要后面（`core/checkpoint-close.txt`）。
    pub checkpoint_close: String,
    /// 包装的结尾，在代码写的几段、重读的文件后面（`core/checkpoint-end.txt`，施工 6-5 从 close 里拆出来）。以前造的
    /// 快照里没有，是空的：那时的 close 里本来就带着那一句。
    pub checkpoint_end: String,
    /// 重读的文件那一块的头尾（`core/compaction/restored-open.txt`、`restored-close.txt`，施工 6-5）；没有的不写重读的
    /// 文件。
    pub restored: Option<RestoredWrap>,
    /// 回合没走完时，排在下一条人的消息前面的那一句。
    pub turn_ended: TurnEndedTexts,
    /// 压缩的摘要指令的正文，摘要请求的最后一块（`core/compaction/summarize-task.txt`，施工 6-2 上）。
    pub summarize_task: String,
    /// 截短重试的摘要请求、留下的第一条是助手的，前面补的那一条 user（`core/compaction/truncated.txt`，施工 6-6 中）。
    /// 以前造的快照里没有，是空的：那些会话不截短。
    pub truncated: String,
    /// 隔离式的摘要请求那一句 system（`core/compaction/summarize-system.txt`，施工 6-6 下）。以前造的快照里没有，是空的：
    /// 那些会话不改走隔离式。
    pub summarize_system: String,
    /// 手动压缩附了要求的，要求前面那一行（`core/compaction/summarize-instructions.txt`，施工 6-8）。以前造的快照里
    /// 没有，是空的。
    pub summarize_instructions: String,
    /// 摘要指令的最后一句：只回草稿和摘要，不许调工具（`core/compaction/summarize-end.txt`，施工 6-8 从正文里拆出来）。
    /// 以前造的快照里没有，是空的：那时的正文里本来就带着这一句，拼出来一字不差。
    pub summarize_end: String,
    /// 两种回报的写法（`core/jobs/`，施工 7-2）。以前造的快照里没有，是没有：那些会话派不出任务，也就没有回报。
    pub jobs: Option<JobTexts>,
}

impl Texts {
    /// 摘要指令（`compaction.md` 第三条第 3 条、第七条第 3 条，施工 6-8）：正文，附了要求的接要求前面那一行和要求，
    /// 最后是结尾那一句。要求原样放、不转义：是人亲口说的，和人发的消息一样（Claude Code 也原样接）；末尾没有换行的
    /// 补一个，结尾那一句才另起一段。照 Claude Code 的次序，最后一句还是不许调工具。
    pub(crate) fn instruction(&self, instructions: Option<&str>) -> String {
        let mut text = self.summarize_task.clone();
        if let Some(instructions) = instructions {
            text.push_str(&self.summarize_instructions);
            text.push_str(instructions);
            if !instructions.ends_with('\n') {
                text.push('\n');
            }
        }
        text.push_str(&self.summarize_end);
        text
    }
}

/// 重读的文件那一块的头尾（施工 6-5）：头上写路径，原文夹在中间不转义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredWrap {
    /// 头：字段 `path`。
    pub open: Template,
    /// 尾。
    pub close: String,
}

/// 回合没走完的五句，一种原因一句（`core/turn-ended/<原因>.txt`）。正常走完的不说。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnEndedTexts {
    /// 被人打断（`interrupted.txt`）。
    pub interrupted: String,
    /// 出了错（`error.txt`）。
    pub error: String,
    /// 走到了步数上限（`step_limit.txt`）。
    pub step_limit: String,
    /// 程序崩了，没走完（`aborted.txt`）。
    pub aborted: String,
    /// 被有计划的重启打断（`restarted.txt`）。
    pub restarted: String,
}

impl TurnEndedTexts {
    /// 这个原因要说的那一句。正常走完的不说；不认识的原因是新版本才有的，不知道该怎么说，
    /// 也不说。
    pub(crate) fn for_reason(&self, reason: &EndReason) -> Option<&str> {
        match reason {
            EndReason::Interrupted => Some(&self.interrupted),
            EndReason::Error => Some(&self.error),
            EndReason::StepLimit => Some(&self.step_limit),
            EndReason::Aborted => Some(&self.aborted),
            EndReason::Restarted => Some(&self.restarted),
            EndReason::Completed | EndReason::Other(_) => None,
        }
    }
}

/// 两种回报的写法（施工 7-2，`docs/blueprint/kernel/request.md`「回报」）：一块带标签的事实，开头一行是标签，中间一行
/// 一句，最后是收尾的标签。每一份以一个换行结尾。子代理发来的留言也照这样包一层标签（施工 7-7）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobTexts {
    /// 后台命令结束的标签（`command-open.txt`）：字段 `job`、`title`、`reason`。
    pub command_open: Template,
    /// 退出码（`command-exit.txt`）：字段 `code`。
    pub command_exit: Template,
    /// 被信号杀掉（`command-signal.txt`）：字段 `signal`。
    pub command_signal: Template,
    /// 用时（`command-duration.txt`）：字段 `ms`。
    pub command_duration: Template,
    /// 输出有多少字、怎么看（`command-output.txt`）：字段 `chars`。
    pub command_output: Template,
    /// 收尾（`command-close.txt`）。
    pub command_close: String,
    /// 子代理的回报的标签（`subagent-open.txt`）：字段 `job`、`title`、`reason`。
    pub subagent_open: Template,
    /// 人插过话（`subagent-person.txt`）。
    pub subagent_person: String,
    /// 正文截过（`subagent-truncated.txt`）。
    pub subagent_truncated: String,
    /// 一个字都没说（`subagent-silent.txt`）。
    pub subagent_silent: String,
    /// 收尾（`subagent-close.txt`）。
    pub subagent_close: String,
    /// 子代理发来的留言的标签（`subagent-message-open.txt`，施工 7-7）：字段 `job`、`title`。以前造的快照里没有，是空的。
    pub subagent_message_open: Template,
    /// 留言的收尾（`subagent-message-close.txt`，施工 7-7）。以前造的快照里没有，是空的。
    pub subagent_message_close: String,
}
